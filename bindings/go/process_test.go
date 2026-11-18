package zumen

import (
	"bufio"
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"testing"
	"time"
)

// 子プロセスのエンジン（#462）を、自作の応答器と CLI で確かめる。
//
// 応答器は、この試験の実行ファイル自身を ZUMEN_FAKE_SERVE=<振る舞い> で起こしたもの（TestMain）。JavaScript の
// bindings/js/test/fixtures/process-child.mjs と同じ振る舞いを持つ。ZUMEN_CLI に CLI の実行ファイルを渡すと、適合テストの
// 全ケースを子プロセスのエンジンでも流す。

// sessionAPI は、wasm と子プロセスのセッションが共に持つメソッド。片方だけを変えたら、ここで組み立てが止まる。
type sessionAPI interface {
	Push(ctx context.Context, name string, data []byte) error
	PushBytes(ctx context.Context, name, data []byte) error
	Call(ctx context.Context, request any) (*Response, error)
	CallBytes(ctx context.Context, request []byte) ([]byte, error)
	CallText(ctx context.Context, request string) (string, error)
	Take(ctx context.Context, blob uint32) ([]byte, error)
	Cancel()
	Close(ctx context.Context) error
	IsClosed() bool
}

var (
	_ sessionAPI = (*Session)(nil)
	_ sessionAPI = (*ProcessSession)(nil)
)

const fakeServe = "ZUMEN_FAKE_SERVE"

func TestMain(m *testing.M) {
	if mode := os.Getenv(fakeServe); mode != "" && len(os.Args) == 3 && os.Args[1] == "serve" && os.Args[2] == "--stdio" {
		serveFake(mode)
		os.Exit(0)
	}
	os.Exit(m.Run())
}

// serveFake は、通信だけを検査する JSON-RPC の応答器。
func serveFake(mode string) {
	var mu sync.Mutex
	write := func(data []byte) {
		mu.Lock()
		defer mu.Unlock()
		_, _ = os.Stdout.Write(data)
	}
	reply := func(id uint64, result any) []byte {
		line, _ := json.Marshal(map[string]any{"jsonrpc": "2.0", "id": id, "result": result})
		return append(line, '\n')
	}
	waiting := uint64(0)
	scanner := bufio.NewScanner(os.Stdin)
	scanner.Buffer(nil, 8<<20)
	for scanner.Scan() {
		var message struct {
			ID     uint64         `json:"id"`
			Method string         `json:"method"`
			Params map[string]any `json:"params"`
		}
		_ = json.Unmarshal(scanner.Bytes(), &message)
		id := message.ID
		switch {
		case message.Method == "push" && mode == "slow-push":
			time.Sleep(300 * time.Millisecond)
			write(reply(id, map[string]string{"status": "ok"}))
			continue
		case message.Method == "session_new" || message.Method == "push":
			write(reply(id, map[string]string{"status": "ok"}))
			continue
		case message.Method == "cancel":
			if mode == "hold" && waiting != 0 {
				write(append(reply(id, map[string]any{}), reply(waiting, map[string]string{"status": "failed", "response": `{"cancelled":true}`})...))
				waiting = 0
			} else {
				write(reply(id, map[string]any{}))
			}
			continue
		}
		switch mode {
		case "exit":
			os.Exit(7)
		case "truncated":
			write([]byte(`{"jsonrpc":`))
			os.Exit(0)
		case "invalid-utf8":
			write([]byte{255, '\n'})
		case "json-error":
			line, _ := json.Marshal(map[string]any{"jsonrpc": "2.0", "id": id, "error": map[string]int{"code": -32600}})
			write(append(line, '\n'))
		case "wrong-id":
			write(reply(id+1, map[string]string{"status": "ok"}))
		case "oversized":
			write(bytes.Repeat([]byte("x"), 2048))
		case "unknown-status":
			write(reply(id, map[string]string{"status": "unknown"}))
		case "null-response":
			write(reply(id, map[string]any{"status": "ok", "response": nil}))
		case "no-response":
			write(reply(id, map[string]string{"status": "ok"}))
		case "bad-base64":
			write(reply(id, map[string]string{"status": "ok", "base64": "AA=A"}))
		case "panicked":
			write(reply(id, map[string]string{"status": "panicked", "response": `{"panic":true}`}))
		case "hold", "silent":
			waiting = id
		default:
			if message.Method == "take" {
				write(reply(id, map[string]string{"status": "ok", "base64": base64.StdEncoding.EncodeToString(bytes.Repeat([]byte{165}, 256<<10))}))
				continue
			}
			line := reply(id, map[string]string{"status": "ok", "response_base64": message.Params["request_base64"].(string)})
			if mode == "fragmented" {
				for i := range line {
					write(line[i : i+1])
				}
			} else {
				write(line)
			}
		}
	}
	if mode == "ignore-eof" {
		// 標準入力が閉じても終わらない子プロセス
		time.Sleep(30 * time.Second)
	}
}

// openFake は、応答器を起こす。起動の時間を期限に含めないよう、Timeout は作った後で当てる。
func openFake(t *testing.T, mode string, limits ProcessOptions) *ProcessSession {
	t.Helper()
	t.Setenv(fakeServe, mode)
	session, err := OpenProcess(context.Background(), os.Args[0], nil, &ProcessOptions{Timeout: 10 * time.Second, MaxMessageBytes: limits.MaxMessageBytes})
	if err != nil {
		t.Fatal(err)
	}
	if limits.Timeout > 0 {
		// 期限は呼び出す側のゴルーチンだけが読む
		session.options.Timeout = limits.Timeout
	}
	t.Cleanup(func() { _ = session.Close(context.Background()) })
	return session
}

func statusOf(err error) string {
	var zumenError *ZumenError
	if errors.As(err, &zumenError) {
		return zumenError.Status()
	}
	return ""
}

func codeOf(err error) string {
	var processError *ProcessError
	if errors.As(err, &processError) {
		return processError.Code
	}
	return ""
}

// closeAndCheck は、セッションを閉じ、子プロセスが終わったことを確かめる。
func closeAndCheck(t *testing.T, session *ProcessSession) {
	t.Helper()
	if err := session.Close(context.Background()); err != nil {
		t.Fatal(err)
	}
	select {
	case <-session.exited:
	default:
		t.Error("Close の後も子プロセスが終わっていない")
	}
}

// waitActive は、呼び出しの要求を書き終えるまで待ち、書き終えたらすぐに戻る。
func waitActive(t *testing.T, session *ProcessSession) {
	t.Helper()
	for range 5000 {
		session.mu.Lock()
		active := session.active
		session.mu.Unlock()
		if active != nil {
			<-active.written
			return
		}
		time.Sleep(time.Millisecond)
	}
	t.Fatal("呼び出しが始まらない")
}

func TestProcessBytes(t *testing.T) {
	ctx := context.Background()
	session := openFake(t, "echo", ProcessOptions{})
	data := []byte{0, 255, 1, 254}
	if got, err := session.CallBytes(ctx, data); err != nil || !bytes.Equal(got, data) {
		t.Errorf("任意のバイト列が %v（%v）", got, err)
	}
	if got, err := session.CallText(ctx, `{"text":"日本語 😀"}`); err != nil || got != `{"text":"日本語 😀"}` {
		t.Errorf("文字列が %q（%v）", got, err)
	}
	blob, err := session.Take(ctx, 0)
	if err != nil || len(blob) != 256<<10 || bytes.Count(blob, []byte{165}) != len(blob) {
		t.Errorf("blob の長さが %d（%v）", len(blob), err)
	}
	session.Cancel()
	closeAndCheck(t, session)
	if err := session.Close(ctx); err != nil {
		t.Error(err)
	}
	if _, err := session.CallBytes(ctx, []byte("{}")); !session.IsClosed() || statusOf(err) != "invalid" {
		t.Errorf("閉じた後の call が %v（期待は invalid）", err)
	}
}

func TestProcessFragmentedAndLimit(t *testing.T) {
	ctx := context.Background()
	session := openFake(t, "fragmented", ProcessOptions{MaxMessageBytes: 1024})
	if err := session.Push(ctx, "input", make([]byte, 1024)); codeOf(err) != "limit" || session.Broken() {
		t.Errorf("上限を超える push が %v", err)
	}
	if got, err := session.CallText(ctx, `{"text":"日本語"}`); err != nil || got != `{"text":"日本語"}` {
		t.Errorf("1 バイトずつ届く応答が %q（%v）", got, err)
	}
	closeAndCheck(t, session)
}

func TestProcessCancelAndBusy(t *testing.T) {
	ctx := context.Background()
	session := openFake(t, "hold", ProcessOptions{})
	done := make(chan error, 1)
	go func() {
		_, err := session.CallBytes(ctx, []byte("{}"))
		done <- err
	}()
	waitActive(t, session)
	if err := session.Push(ctx, "input", []byte("x")); statusOf(err) != "busy" {
		t.Errorf("処理中の push が %v（期待は busy）", err)
	}
	if _, err := session.Take(ctx, 0); statusOf(err) != "busy" {
		t.Errorf("処理中の take が %v（期待は busy）", err)
	}
	session.Cancel()
	session.Cancel()
	err := <-done
	var failed *ZumenError
	if !errors.As(err, &failed) || failed.Status() != "failed" || string(failed.Data) != `{"cancelled":true}` || !errors.Is(err, context.Canceled) {
		t.Errorf("中断した call が %v", err)
	}
	if session.Broken() {
		t.Error("中断に応じたのに、壊れたセッションになった")
	}
	// コンテキストの取り消しでも、中断の合図を送る
	canceled, cancel := context.WithCancel(ctx)
	go func() {
		_, err := session.CallBytes(canceled, []byte("{}"))
		done <- err
	}()
	waitActive(t, session)
	cancel()
	if err := <-done; statusOf(err) != "failed" || !errors.Is(err, context.Canceled) {
		t.Errorf("取り消したコンテキストの call が %v", err)
	}
	// 始める前に取り消したコンテキストは、送らずに返す
	if _, err := session.CallBytes(canceled, []byte("{}")); statusOf(err) != "failed" || !errors.Is(err, context.Canceled) {
		t.Errorf("取り消し済みのコンテキストの call が %v", err)
	}
	if err := session.Push(ctx, "input", []byte("x")); err != nil || session.Broken() {
		t.Errorf("中断の後の push が %v", err)
	}
	closeAndCheck(t, session)
}

func TestProcessFailures(t *testing.T) {
	ctx := context.Background()
	for _, test := range []struct{ mode, code string }{
		{"exit", "io"}, {"truncated", "io"}, {"invalid-utf8", "protocol"}, {"json-error", "protocol"},
		{"wrong-id", "protocol"}, {"unknown-status", "protocol"}, {"no-response", "protocol"}, {"null-response", "protocol"},
		{"bad-base64", "protocol"}, {"oversized", "limit"},
	} {
		t.Run(test.mode, func(t *testing.T) {
			session := openFake(t, test.mode, ProcessOptions{MaxMessageBytes: 1024})
			var err error
			if test.mode == "bad-base64" {
				_, err = session.Take(ctx, 0)
			} else {
				_, err = session.CallBytes(ctx, []byte("{}"))
			}
			if codeOf(err) != test.code || !session.Broken() {
				t.Errorf("失敗が %v（期待は %s）", err, test.code)
			}
			if _, err := session.CallBytes(ctx, []byte("{}")); codeOf(err) != test.code {
				t.Errorf("壊れた後の call が %v", err)
			}
			session.Cancel()
			closeAndCheck(t, session)
		})
	}
}

func TestProcessTimeout(t *testing.T) {
	ctx := context.Background()
	// 期限を超えた
	session := openFake(t, "silent", ProcessOptions{Timeout: 200 * time.Millisecond})
	if _, err := session.CallBytes(ctx, []byte("{}")); codeOf(err) != "timeout" || !session.Broken() {
		t.Errorf("期限を超えた call が %v", err)
	}
	closeAndCheck(t, session)
	// 中断の合図に応じない
	session = openFake(t, "silent", ProcessOptions{Timeout: 5 * time.Second})
	done := make(chan error, 1)
	go func() {
		_, err := session.CallBytes(ctx, []byte("{}"))
		done <- err
	}()
	waitActive(t, session)
	session.Cancel()
	if err := <-done; codeOf(err) != "timeout" || !session.Broken() {
		t.Errorf("中断に応じない call が %v", err)
	}
	closeAndCheck(t, session)
	// 取り消したコンテキストに応じない
	session = openFake(t, "silent", ProcessOptions{Timeout: 5 * time.Second})
	canceled, cancel := context.WithCancel(ctx)
	go func() {
		_, err := session.CallBytes(canceled, []byte("{}"))
		done <- err
	}()
	waitActive(t, session)
	cancel()
	if err := <-done; codeOf(err) != "timeout" || !errors.Is(err, context.Canceled) {
		t.Errorf("取り消しに応じない call が %v", err)
	}
	closeAndCheck(t, session)
}

func TestProcessRequestLimit(t *testing.T) {
	ctx := context.Background()
	session := openFake(t, "echo", ProcessOptions{MaxMessageBytes: 1024})
	// base64 では上限に入るが、JSON-RPC の枠を足すと超える。送らずに返し、セッションは使い続けられる
	if _, err := session.CallBytes(ctx, make([]byte, 765)); codeOf(err) != ProcessLimit || session.Broken() {
		t.Errorf("枠で上限を超える call が %v", err)
	}
	if _, err := session.CallBytes(ctx, make([]byte, 600)); err != nil {
		t.Errorf("上限に入る call が %v", err)
	}
	// 閉じたセッションには、上限より先に invalid を返す
	closeAndCheck(t, session)
	if err := session.Push(ctx, "a", make([]byte, 4096)); statusOf(err) != "invalid" {
		t.Errorf("閉じた後の大きな push が %v（期待は invalid）", err)
	}
}

func TestProcessPushIgnoresLateCancel(t *testing.T) {
	session := openFake(t, "slow-push", ProcessOptions{Timeout: -1})
	ctx, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
	defer cancel()
	// コアは push を中断しないので、送った後の取り消しでは子プロセスを止めない
	if err := session.Push(ctx, "a", []byte("x")); err != nil || session.Broken() {
		t.Errorf("取り消した後に終わった push が %v", err)
	}
	closeAndCheck(t, session)
}

func TestProcessCloseKillsStuckChild(t *testing.T) {
	session := openFake(t, "ignore-eof", ProcessOptions{})
	canceled, cancel := context.WithCancel(context.Background())
	cancel()
	if err := session.Close(canceled); !errors.Is(err, context.Canceled) {
		t.Errorf("取り消したコンテキストの Close が %v", err)
	}
	start := time.Now()
	closeAndCheck(t, session)
	if elapsed := time.Since(start); elapsed > 5*time.Second {
		t.Errorf("終わらない子プロセスを止めるのに %v かかった", elapsed)
	}
}

func TestProcessCloseWhileRunning(t *testing.T) {
	session := openFake(t, "hold", ProcessOptions{})
	done := make(chan error, 1)
	go func() {
		_, err := session.CallBytes(context.Background(), []byte("{}"))
		done <- err
	}()
	waitActive(t, session)
	closeAndCheck(t, session)
	if err := <-done; codeOf(err) != "closed" {
		t.Errorf("処理中に閉じた call が %v", err)
	}
}

func TestProcessPanicked(t *testing.T) {
	ctx := context.Background()
	session := openFake(t, "panicked", ProcessOptions{})
	_, err := session.CallBytes(ctx, []byte("{}"))
	var failed *ZumenError
	if !errors.As(err, &failed) || failed.Status() != "panicked" || string(failed.Data) != `{"panic":true}` || !session.Broken() {
		t.Errorf("パニックの応答が %v", err)
	}
	if _, err := session.Take(ctx, 0); statusOf(err) != "panicked" {
		t.Errorf("パニックの後の take が %v", err)
	}
	closeAndCheck(t, session)
}

func TestProcessOpenErrors(t *testing.T) {
	ctx := context.Background()
	if _, err := OpenProcess(ctx, filepath.Join(t.TempDir(), "no-such-engine"), nil, nil); codeOf(err) != "spawn" {
		t.Errorf("ない実行ファイルが %v（期待は spawn）", err)
	}
	for _, limits := range []ProcessOptions{{MaxMessageBytes: -1}, {MaxMessageBytes: 1}, {MaxMessageBytes: 2 << 30}} {
		if _, err := OpenProcess(ctx, os.Args[0], nil, &limits); err == nil || codeOf(err) != "" {
			t.Errorf("使えない上限 %+v が %v", limits, err)
		}
	}
	if _, err := OpenProcess(ctx, "", nil, nil); err == nil {
		t.Error("空の道筋で起動した")
	}
}

// cliPath は、ZUMEN_CLI の CLI。なければ試験を飛ばす。
func cliPath(t *testing.T) string {
	t.Helper()
	path := os.Getenv("ZUMEN_CLI")
	if path == "" {
		t.Skip("ZUMEN_CLI に CLI の実行ファイルを渡すと流す")
	}
	return path
}

func TestProcessConformance(t *testing.T) {
	executable := cliPath(t)
	limits := &ProcessOptions{Timeout: 2 * time.Minute}
	runConformance(t, "Go の子プロセスのエンジンの適合テスト", func(ctx context.Context, options []byte) (engineSession, error) {
		session, err := OpenProcessBytes(ctx, executable, options, limits)
		if err != nil {
			return nil, err
		}
		return session, nil
	})
}

func TestProcessCLI(t *testing.T) {
	executable := cliPath(t)
	ctx := context.Background()
	a, err := OpenProcess(ctx, executable, &SessionOptions{V: new(SchemaMajor)}, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer a.Close(ctx)
	b, err := OpenProcess(ctx, executable, nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer b.Close(ctx)
	response, err := a.Call(ctx, CapabilitiesRequest{V: new(SchemaMajor)})
	if result, rerr := ResultOf[CapabilitiesResult](response); err != nil || rerr != nil || result == nil || result.Package == nil {
		t.Errorf("capabilities の応答が %+v（%v）", response, err)
	}
	// 入力はセッションごとに分かれる
	if err := a.Push(ctx, "input.dxf", []byte("0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n")); err != nil {
		t.Fatal(err)
	}
	if _, err := a.Call(ctx, OpenRequest{V: new(SchemaMajor), Input: new("input.dxf")}); err != nil {
		t.Errorf("渡した入力を開けない: %v", err)
	}
	_, err = b.Call(ctx, OpenRequest{V: new(SchemaMajor), Input: new("input.dxf")})
	var failed *ZumenError
	if !errors.As(err, &failed) || failed.Status() != "failed" || failed.Response() == nil || len(failed.Response().Diagnostics) == 0 {
		t.Errorf("ほかのセッションの入力を開く call が %v（期待は診断のある failed）", err)
	}
	if !strings.Contains(err.Error(), "zm_call") {
		t.Errorf("失敗の文に関数の名前がない: %v", err)
	}
	if err := a.Close(ctx); err != nil {
		t.Fatal(err)
	}
	if _, err := b.CallText(ctx, `{"v":1,"op":"capabilities"}`); err != nil {
		t.Errorf("ほかのセッションを閉じた後の call が %v", err)
	}
	// 呼び出しの直後の中断も、CLI が受けてコアを止める。セッションは使い続けられる
	var dxf strings.Builder
	dxf.WriteString("0\nSECTION\n2\nENTITIES\n")
	for i := range 300000 {
		dxf.WriteString("0\nLINE\n8\n0\n10\n" + strconv.Itoa(i%1000) + "\n20\n" + strconv.Itoa(i/1000) + "\n11\n1\n21\n2\n")
	}
	dxf.WriteString("0\nENDSEC\n0\nEOF\n")
	if err := b.Push(ctx, "large.dxf", []byte(dxf.String())); err != nil {
		t.Fatal(err)
	}
	for round := range 4 {
		done := make(chan error, 1)
		callCtx, cancelCall := context.WithCancel(ctx)
		go func() {
			_, err := b.Call(callCtx, OpenRequest{V: new(SchemaMajor), Input: new("large.dxf")})
			done <- err
		}()
		waitActive(t, b)
		if round%2 == 0 {
			b.Cancel()
		} else {
			cancelCall()
		}
		err := <-done
		cancelCall()
		if statusOf(err) != "failed" || !errors.Is(err, context.Canceled) || b.Broken() {
			t.Errorf("回 %d: 書いた直後に中断した call が %v", round, err)
		}
		if _, err := b.CallText(ctx, `{"v":1,"op":"capabilities"}`); err != nil {
			t.Errorf("回 %d: 中断の後の call が %v", round, err)
		}
	}
	// 設定が読めなければ、セッションを作らない
	if _, err := OpenProcessBytes(ctx, executable, []byte("{"), nil); statusOf(err) != "invalid" {
		t.Errorf("読めない設定が %v（期待は invalid）", err)
	}
}
