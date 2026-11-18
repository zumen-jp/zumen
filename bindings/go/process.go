package zumen

// 子プロセスのエンジン（#462・決定 0201）。CLI の serve --stdio を起こし、標準入出力の JSON-RPC で API の関数を呼ぶ。
// JavaScript の bindings/js/src/process.js と同じメソッド・上限・失敗の分け方にする。

import (
	"bufio"
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os/exec"
	"sync"
	"sync/atomic"
	"time"
	"unicode/utf8"
)

const (
	defaultProcessTimeout = 30 * time.Second
	defaultMessageBytes   = 64 << 20
	minMessageBytes       = 1024
	maxMessageBytes       = 1 << 30
	// cancelGrace は、中断の合図や終了の後、子プロセスの応答や終了を待つ時間。
	cancelGrace = time.Second
)

// ProcessError の Code の値。JavaScript の ProcessError の code と同じ。
const (
	// ProcessSpawn: 子プロセスを起動できない
	ProcessSpawn = "spawn"
	// ProcessIO: 子プロセスと読み書きできない、子プロセスが終わった
	ProcessIO = "io"
	// ProcessLimit: 要求か応答の 1 行が MaxMessageBytes を超える
	ProcessLimit = "limit"
	// ProcessProtocol: 子プロセスの応答が JSON-RPC の契約と違う
	ProcessProtocol = "protocol"
	// ProcessTimeout: 応答の期限を超えた、中断の合図に応答しない
	ProcessTimeout = "timeout"
	// ProcessClosed: 処理中にセッションを閉じた
	ProcessClosed = "closed"
)

// ProcessOptions は、子プロセスのエンジンの設定。0 の値は既定にする。
type ProcessOptions struct {
	// Timeout は、1 つの要求の応答を待つ時間。既定は 30 秒、負の値は期限なし。超えたら子プロセスを止め、セッションを
	// 壊れたものにする。
	Timeout time.Duration

	// MaxMessageBytes は、要求と応答の 1 行の大きさの上限（バイト、改行を含む）。既定は 64 MiB、1024 から 1 GiB まで。
	MaxMessageBytes int
}

// ProcessError は、子プロセスの起動・通信・上限による失敗。コアが返した状態は ZumenError で返す。
type ProcessError struct {
	// Code は、失敗の種類（Process で始まる定数のどれか）。
	Code string

	// Message は、説明。
	Message string

	// Err は、元の失敗（起動の失敗・読み書きの失敗・取り消したコンテキストの理由など）。なければ nil。
	Err error
}

func (e *ProcessError) Error() string {
	if e.Err != nil {
		return fmt.Sprintf("子プロセスのエンジン（%s）: %s: %v", e.Code, e.Message, e.Err)
	}
	return fmt.Sprintf("子プロセスのエンジン（%s）: %s", e.Code, e.Message)
}

func (e *ProcessError) Unwrap() error {
	return e.Err
}

// reply は、待っている要求への答え。
type reply struct {
	result map[string]json.RawMessage
	err    error
}

// ProcessSession は、1 つの子プロセスと、その中のコアのセッション。メソッドは Session と同じ名前・引数で、同じ状態を返す。
//
// 通常の呼び出しは一度に 1 つで、重なった呼び出しは busy の ZumenError になる。Cancel・Close は、どのゴルーチンからも呼べる。
// Session と違い、中断した Call の後もセッションは使える（コアが中断して failed を返すため）。中断の合図に 1 秒で
// 応答しない子プロセスは止め、セッションを壊れたもの（Broken）にする。
type ProcessSession struct {
	cmd     *exec.Cmd
	stdin   io.WriteCloser
	options ProcessOptions
	exited  chan struct{}
	// write は、標準入力に 1 行ずつ書くための鍵。mu より先に取る
	write sync.Mutex

	mu       sync.Mutex
	pending  map[uint64]chan reply
	next     uint64
	active   *activeCall
	failure  error
	panicked bool
	closed   bool
}

// activeCall は、動いている通常の呼び出し。
type activeCall struct {
	method string
	// written は、要求を書き終えたか、書かずに終えたら閉じる
	written chan struct{}
	once    sync.Once
	// cancelled は、Cancel で中断を求めたか
	cancelled atomic.Bool
}

func (a *activeCall) markWritten() {
	a.once.Do(func() { close(a.written) })
}

// OpenProcess は、CLI（executable）を serve --stdio で起こし、設定を値で渡してセッションを作る。options・process が nil
// なら既定。子プロセスはセッションごとに 1 つで、シェルを介さず、標準エラー出力は捨てる。使い終わったら Close で止める。
func OpenProcess(ctx context.Context, executable string, options *SessionOptions, process *ProcessOptions) (*ProcessSession, error) {
	if options == nil {
		return OpenProcessBytes(ctx, executable, nil, process)
	}
	data, err := json.Marshal(options)
	if err != nil {
		return nil, err
	}
	return OpenProcessBytes(ctx, executable, data, process)
}

// OpenProcessBytes は、設定を JSON の UTF-8 のバイト列で渡して、子プロセスのセッションを作る。長さが 0 なら既定の設定。
func OpenProcessBytes(ctx context.Context, executable string, options []byte, process *ProcessOptions) (*ProcessSession, error) {
	resolved, err := processOptionsOf(process)
	if err != nil {
		return nil, err
	}
	if executable == "" {
		return nil, errors.New("CLI の実行ファイルの道筋を渡す")
	}
	cmd := exec.Command(executable, "serve", "--stdio")
	hideWindow(cmd)
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return nil, &ProcessError{Code: ProcessSpawn, Message: "子プロセスを起動できない", Err: err}
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, &ProcessError{Code: ProcessSpawn, Message: "子プロセスを起動できない", Err: err}
	}
	if err := cmd.Start(); err != nil {
		return nil, &ProcessError{Code: ProcessSpawn, Message: "子プロセスを起動できない", Err: err}
	}
	session := &ProcessSession{
		cmd:     cmd,
		stdin:   stdin,
		options: resolved,
		exited:  make(chan struct{}),
		pending: map[uint64]chan reply{},
		next:    1,
	}
	go session.read(stdout)
	if _, err := session.invoke(ctx, "session_new", "zm_session_new", map[string]any{"options_base64": base64.StdEncoding.EncodeToString(options)}); err != nil {
		_ = session.Close(context.WithoutCancel(ctx))
		return nil, err
	}
	return session, nil
}

func processOptionsOf(process *ProcessOptions) (ProcessOptions, error) {
	resolved := ProcessOptions{Timeout: defaultProcessTimeout, MaxMessageBytes: defaultMessageBytes}
	if process == nil {
		return resolved, nil
	}
	if process.Timeout != 0 {
		resolved.Timeout = process.Timeout
	}
	if process.MaxMessageBytes != 0 {
		if process.MaxMessageBytes < minMessageBytes || process.MaxMessageBytes > maxMessageBytes {
			return resolved, errors.New("MaxMessageBytes は 1024 から 1073741824 までにする")
		}
		resolved.MaxMessageBytes = process.MaxMessageBytes
	}
	return resolved, nil
}

// IsClosed は、Close を呼んだか。
func (s *ProcessSession) IsClosed() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.closed
}

// Broken は、通信の失敗かコアのパニックで、子プロセスを止めたか。壊れたセッションへの呼び出しは、同じ失敗を返す。
func (s *ProcessSession) Broken() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.failure != nil || s.panicked
}

// read は、子プロセスの応答を 1 行ずつ読み、待っている要求に渡す。読み終えたら、子プロセスの終了を待つ。
func (s *ProcessSession) read(stdout io.Reader) {
	defer func() {
		_ = s.cmd.Wait()
		s.mu.Lock()
		quiet := s.closed || s.panicked || s.failure != nil
		s.mu.Unlock()
		if !quiet {
			s.fail(&ProcessError{Code: ProcessIO, Message: "子プロセスが終了した"})
		}
		close(s.exited)
	}()
	// 上限を超えたら改行を待たずに気づくように、バッファーを上限の大きさにする（64 KiB まで）
	reader := bufio.NewReaderSize(stdout, min(64<<10, s.options.MaxMessageBytes))
	// drain は、閉じた後の応答を読み捨て、子プロセスが書けずに止まらないようにする
	drain := func() bool {
		if s.IsClosed() {
			_, _ = io.Copy(io.Discard, reader)
			return true
		}
		return false
	}
	var line []byte
	for {
		chunk, err := reader.ReadSlice('\n')
		// 行の長さは、末尾の改行を含めて数える（CLI の読み方と同じ）
		if len(line)+len(chunk) > s.options.MaxMessageBytes {
			if !drain() {
				s.fail(&ProcessError{Code: ProcessLimit, Message: "応答が MaxMessageBytes を超える"})
			}
			return
		}
		line = append(line, chunk...)
		if errors.Is(err, bufio.ErrBufferFull) {
			continue
		}
		if err != nil {
			s.mu.Lock()
			quiet := s.closed || s.panicked
			s.mu.Unlock()
			if !quiet {
				s.fail(&ProcessError{Code: ProcessIO, Message: "子プロセスの応答が途切れた", Err: err})
			}
			return
		}
		if perr := s.dispatch(line[:len(line)-1]); perr != nil {
			if !drain() {
				s.fail(&ProcessError{Code: ProcessProtocol, Message: "子プロセスの応答が JSON-RPC の契約と違う", Err: perr})
			}
			return
		}
		if cap(line) > 64<<10 {
			line = nil
		} else {
			line = line[:0]
		}
	}
}

// dispatch は、1 行の応答を、送った要求に渡す。
func (s *ProcessSession) dispatch(line []byte) error {
	if !utf8.Valid(line) {
		return errors.New("UTF-8 でない")
	}
	var message map[string]json.RawMessage
	if err := json.Unmarshal(line, &message); err != nil {
		return err
	}
	var version string
	var id uint64
	if json.Unmarshal(message["jsonrpc"], &version) != nil || version != "2.0" || json.Unmarshal(message["id"], &id) != nil {
		return errors.New("jsonrpc か id が違う")
	}
	if _, ok := message["error"]; ok {
		return errors.New("エラーの応答")
	}
	var result map[string]json.RawMessage
	if raw := bytes.TrimSpace(message["result"]); len(raw) == 0 || raw[0] != '{' || json.Unmarshal(raw, &result) != nil {
		return errors.New("result がオブジェクトでない")
	}
	s.mu.Lock()
	ch, ok := s.pending[id]
	delete(s.pending, id)
	s.mu.Unlock()
	if !ok {
		return errors.New("応答が送った要求に対応しない")
	}
	ch <- reply{result: result}
	return nil
}

// fail は、通信の失敗で、待っている要求をすべて失敗にし、子プロセスを止める。
func (s *ProcessSession) fail(err error) {
	s.mu.Lock()
	if s.closed || s.failure != nil || s.panicked {
		s.mu.Unlock()
		return
	}
	s.failure = err
	s.rejectAll(err)
	s.mu.Unlock()
	_ = s.cmd.Process.Kill()
}

// rejectAll は、待っている要求をすべて err で終える。s.mu を持って呼ぶ。
func (s *ProcessSession) rejectAll(err error) {
	for id, ch := range s.pending {
		ch <- reply{err: err}
		delete(s.pending, id)
	}
}

// check は、通常の呼び出しを始められるか。s.mu を持って呼ぶ。
func (s *ProcessSession) check(operation string) error {
	if err := s.unusable(operation); err != nil {
		return err
	}
	if s.active != nil {
		return &ZumenError{Operation: operation, Code: StatusBusy}
	}
	return nil
}

// unusable は、閉じたか壊れたセッションの失敗。使えるなら nil。s.mu を持って呼ぶ。
func (s *ProcessSession) unusable(operation string) error {
	switch {
	case s.closed:
		return &ZumenError{Operation: operation, Code: StatusInvalid}
	case s.panicked:
		return &ZumenError{Operation: operation, Code: StatusPanicked}
	case s.failure != nil:
		return s.failure
	}
	return nil
}

// precheck は、要求を作る前に、呼び出しを始められるかを確かめる。
func (s *ProcessSession) precheck(operation string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.check(operation)
}

// encode は、要求を 1 行（改行を含む）にする。上限を超えるなら limit の ProcessError。
func (s *ProcessSession) encode(id uint64, method string, params any) ([]byte, error) {
	line, err := json.Marshal(map[string]any{"jsonrpc": "2.0", "id": id, "method": method, "params": params})
	if err != nil {
		return nil, err
	}
	if len(line)+1 > s.options.MaxMessageBytes {
		return nil, &ProcessError{Code: ProcessLimit, Message: "要求が MaxMessageBytes を超える"}
	}
	return append(line, '\n'), nil
}

// register は、答えを待つ要求を登録する。s.mu を持って呼ぶ。
func (s *ProcessSession) register(id uint64, operation string) (chan reply, error) {
	if s.closed {
		return nil, &ProcessError{Code: ProcessClosed, Message: "処理中にセッションを閉じた"}
	}
	if err := s.unusable(operation); err != nil {
		return nil, err
	}
	ch := make(chan reply, 1)
	s.pending[id] = ch
	return ch, nil
}

// invoke は、通常の呼び出しを 1 つ送り、答えのバイト列（応答か blob）を返す。
//
// call のときは、ctx が終わると中断の合図を送り、要求を書き終えてから 1 秒待つ。ほかの関数はコアが中断しないので、
// ctx は送る前にだけ確かめ、送った後は答えか期限を待つ。
func (s *ProcessSession) invoke(ctx context.Context, method, operation string, params map[string]any) ([]byte, error) {
	s.mu.Lock()
	if err := s.check(operation); err != nil {
		s.mu.Unlock()
		return nil, err
	}
	if err := ctx.Err(); err != nil {
		s.mu.Unlock()
		return nil, &ZumenError{Operation: operation, Code: StatusFailed, Err: context.Cause(ctx)}
	}
	active := &activeCall{method: method, written: make(chan struct{})}
	s.active = active
	id := s.next
	s.next++
	s.mu.Unlock()
	defer func() {
		active.markWritten()
		s.mu.Lock()
		if s.active == active {
			s.active = nil
		}
		s.mu.Unlock()
	}()

	line, err := s.encode(id, method, params)
	if err != nil {
		return nil, err
	}
	s.mu.Lock()
	ch, err := s.register(id, operation)
	s.mu.Unlock()
	if err != nil {
		return nil, err
	}
	// 子プロセスが読まなくなっても、呼び出しは期限と取り消しで戻れるように、別のゴルーチンで書く
	go func() {
		s.write.Lock()
		_, err := s.stdin.Write(line)
		s.write.Unlock()
		active.markWritten()
		if err != nil {
			s.fail(&ProcessError{Code: ProcessIO, Message: "子プロセスに書き込めない", Err: err})
		}
	}()

	var deadline <-chan time.Time
	if s.options.Timeout > 0 {
		timer := time.NewTimer(s.options.Timeout)
		defer timer.Stop()
		deadline = timer.C
	}
	done := ctx.Done()
	var grace <-chan time.Time
	var writtenSignal <-chan struct{}
	var cause error
	for {
		select {
		case answer := <-ch:
			if answer.err != nil {
				return nil, answer.err
			}
			if cause == nil && active.cancelled.Load() {
				cause = context.Canceled
			}
			return s.decode(method, operation, answer.result, cause)
		case <-deadline:
			err := &ProcessError{Code: ProcessTimeout, Message: "子プロセスの応答を待つ時間を超えた", Err: cause}
			s.fail(err)
			return nil, err
		case <-done:
			done = nil
			cause = context.Cause(ctx)
			if method == "call" {
				s.signalCancel(active)
				writtenSignal = active.written
			}
		case <-writtenSignal:
			writtenSignal = nil
			timer := time.NewTimer(cancelGrace)
			defer timer.Stop()
			grace = timer.C
		case <-grace:
			err := &ProcessError{Code: ProcessTimeout, Message: "中断の合図を送っても子プロセスが応答しない", Err: cause}
			s.fail(err)
			return nil, err
		}
	}
}

// decode は、答えの状態とバイト列を読む。
func (s *ProcessSession) decode(method, operation string, result map[string]json.RawMessage, cause error) ([]byte, error) {
	var name string
	code := int32(-1)
	if jsonString(result["status"], &name) == nil {
		for candidate := StatusOk; candidate <= StatusPanicked; candidate++ {
			if StatusName(candidate) == name {
				code = candidate
			}
		}
	}
	if code < 0 {
		err := &ProcessError{Code: ProcessProtocol, Message: "子プロセスが未知の状態を返した"}
		s.fail(err)
		return nil, err
	}
	data, err := answerBytes(method, code, result)
	if err != nil {
		perr := &ProcessError{Code: ProcessProtocol, Message: "子プロセスが返したバイト列の形が違う", Err: err}
		s.fail(perr)
		return nil, perr
	}
	if code == StatusPanicked {
		s.mu.Lock()
		if !s.closed && s.failure == nil && !s.panicked {
			s.panicked = true
			s.rejectAll(&ZumenError{Operation: operation, Code: StatusPanicked})
		}
		s.mu.Unlock()
		_ = s.cmd.Process.Kill()
	}
	if code != StatusOk {
		return nil, &ZumenError{Operation: operation, Code: code, Data: data, Err: cause}
	}
	return data, nil
}

// jsonString は、JSON の文字列を読む。null や、ほかの型は誤り。
func jsonString(raw json.RawMessage, out *string) error {
	if len(raw) == 0 || raw[0] != '"' {
		return errors.New("文字列でない")
	}
	return json.Unmarshal(raw, out)
}

// answerBytes は、答えの response・response_base64・base64 をバイト列にする。
func answerBytes(method string, code int32, result map[string]json.RawMessage) ([]byte, error) {
	var data []byte
	text, hasText := result["response"]
	encoded, hasEncoded := result["response_base64"]
	switch {
	case hasText && hasEncoded:
		return nil, errors.New("response と response_base64 の両方がある")
	case hasText:
		var value string
		if err := jsonString(text, &value); err != nil {
			return nil, err
		}
		data = []byte(value)
	case hasEncoded:
		var err error
		if data, err = strictBase64(encoded); err != nil {
			return nil, err
		}
	}
	if code == StatusOk && method == "call" && data == nil {
		return nil, errors.New("response がない")
	}
	if code == StatusOk && method == "take" {
		return strictBase64(result["base64"])
	}
	return data, nil
}

// strictBase64 は、JSON の文字列の base64 を読む。改行や、末尾の使わないビットのあるものは誤り。
func strictBase64(raw json.RawMessage) ([]byte, error) {
	var text string
	if err := jsonString(raw, &text); err != nil {
		return nil, errors.New("base64 が文字列でない")
	}
	if bytes.ContainsAny([]byte(text), "\r\n") {
		return nil, errors.New("base64 に改行がある")
	}
	data, err := base64.StdEncoding.Strict().DecodeString(text)
	if err != nil {
		return nil, err
	}
	if data == nil {
		data = []byte{}
	}
	return data, nil
}

// signalCancel は、動いている Call に中断の合図を送る。合図は Call の要求を書いた後、その Call がまだ動いているときだけ
// 書く。書く鍵を持ったまま確かめるので、次の呼び出しの要求より前に書くか、書かないかのどちらかになる。
func (s *ProcessSession) signalCancel(active *activeCall) {
	go func() {
		select {
		case <-active.written:
		case <-s.exited:
			return
		}
		s.write.Lock()
		defer s.write.Unlock()
		s.mu.Lock()
		if s.active != active {
			s.mu.Unlock()
			return
		}
		id := s.next
		s.next++
		line, err := s.encode(id, "cancel", map[string]any{})
		if err == nil {
			// 合図への答えは、待たずに読み捨てる
			_, err = s.register(id, "zm_cancel")
		}
		s.mu.Unlock()
		if err != nil {
			return
		}
		if _, err := s.stdin.Write(line); err != nil {
			s.fail(&ProcessError{Code: ProcessIO, Message: "子プロセスに書き込めない", Err: err})
		}
	}()
}

// limitedBase64 は、バイト列を base64 にする。上限を超えるなら limit の ProcessError。
func (s *ProcessSession) limitedBase64(parts ...[]byte) ([]string, error) {
	total := 0
	for _, part := range parts {
		total += base64.StdEncoding.EncodedLen(len(part))
	}
	if total >= s.options.MaxMessageBytes {
		return nil, &ProcessError{Code: ProcessLimit, Message: "要求が MaxMessageBytes を超える"}
	}
	out := make([]string, len(parts))
	for i, part := range parts {
		out[i] = base64.StdEncoding.EncodeToString(part)
	}
	return out, nil
}

// Push は、名前のついた入力にバイト列を足す（zm_push）。同じ名前で何回か呼ぶと、後ろに足していく。
func (s *ProcessSession) Push(ctx context.Context, name string, data []byte) error {
	return s.PushBytes(ctx, []byte(name), data)
}

// PushBytes は、入力を、名前のバイト列で渡す。コアは push を中断しないので、ctx は送る前にだけ確かめる。
func (s *ProcessSession) PushBytes(ctx context.Context, name, data []byte) error {
	if err := s.precheck("zm_push"); err != nil {
		return err
	}
	encoded, err := s.limitedBase64(name, data)
	if err != nil {
		return err
	}
	_, err = s.invoke(ctx, "push", "zm_push", map[string]any{"name_base64": encoded[0], "base64": encoded[1]})
	return err
}

// CallBytes は、要求の JSON の UTF-8 を渡し、応答のバイト列を受け取る（zm_call）。
//
// 呼び出し中に ctx が終わると中断の合図を送る。コアが中断して返した失敗は、Err に ctx の理由を入れた ZumenError になる。
func (s *ProcessSession) CallBytes(ctx context.Context, request []byte) ([]byte, error) {
	if err := s.precheck("zm_call"); err != nil {
		return nil, err
	}
	encoded, err := s.limitedBase64(request)
	if err != nil {
		return nil, err
	}
	return s.invoke(ctx, "call", "zm_call", map[string]any{"request_base64": encoded[0]})
}

// CallText は、要求を JSON の文字列で渡し、応答の文字列を受け取る。
func (s *ProcessSession) CallText(ctx context.Context, request string) (string, error) {
	data, err := s.CallBytes(ctx, []byte(request))
	return string(data), err
}

// Call は、要求の値（要求の型か、JSON にできる値）を渡し、応答を読む。
func (s *ProcessSession) Call(ctx context.Context, request any) (*Response, error) {
	data, err := json.Marshal(request)
	if err != nil {
		return nil, err
	}
	data, err = s.CallBytes(ctx, data)
	if err != nil {
		return nil, err
	}
	var response Response
	if err := json.Unmarshal(data, &response); err != nil {
		return nil, err
	}
	return &response, nil
}

// Take は、直前の Call が作ったバイナリの結果を、番号で取り出す（zm_take）。ctx は送る前にだけ確かめる。
func (s *ProcessSession) Take(ctx context.Context, blob uint32) ([]byte, error) {
	return s.invoke(ctx, "take", "zm_take", map[string]any{"blob": blob})
}

// Cancel は、動いている Call に中断の合図を送る。止めた Call は failed の ZumenError（Err は context.Canceled）を返し、
// セッションは使い続けられる。要求を書き終えてから 1 秒で Call が戻らなければ、子プロセスを止める。どのゴルーチンからも
// 呼べ、動いている Call がなければ何もしない。
func (s *ProcessSession) Cancel() {
	s.mu.Lock()
	active := s.active
	s.mu.Unlock()
	if active == nil || active.method != "call" || active.cancelled.Swap(true) {
		return
	}
	s.signalCancel(active)
	go func() {
		select {
		case <-active.written:
		case <-s.exited:
			return
		}
		timer := time.NewTimer(cancelGrace)
		defer timer.Stop()
		select {
		case <-timer.C:
		case <-s.exited:
			return
		}
		s.mu.Lock()
		stuck := s.active == active
		s.mu.Unlock()
		if stuck {
			s.fail(&ProcessError{Code: ProcessTimeout, Message: "中断の合図を送っても子プロセスが応答しない", Err: context.Canceled})
		}
	}()
}

// Close は、セッションを閉じ、子プロセスを終える。標準入力を閉じて 1 秒待ち、終わらなければ止める。処理中の呼び出しは
// closed の ProcessError を返す。2 度目からは、終了を待つだけ。ctx が終わったら、終了を待たずに戻る。
func (s *ProcessSession) Close(ctx context.Context) error {
	s.mu.Lock()
	first := !s.closed
	if first {
		s.closed = true
		s.rejectAll(&ProcessError{Code: ProcessClosed, Message: "処理中にセッションを閉じた"})
	}
	s.mu.Unlock()
	if first {
		_ = s.stdin.Close()
		go func() {
			timer := time.NewTimer(cancelGrace)
			defer timer.Stop()
			select {
			case <-s.exited:
			case <-timer.C:
				_ = s.cmd.Process.Kill()
			}
		}()
	}
	select {
	case <-s.exited:
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}
