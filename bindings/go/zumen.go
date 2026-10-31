// Package zumen は、zumen の wasm のモジュールを wazero で動かす包み（決定 0011・0013・0226）。cgo を使わない。
//
// New でモジュールを読み込み、Core.Open でセッションを作る。Session のメソッドは、境界の関数（schema/boundary.toml）に
// 1 つずつ当たる。要求と応答の型は messages.go にある。
package zumen

import (
	"bytes"
	"context"
	_ "embed"
	"encoding/json"
	"errors"
	"fmt"
	"sync"
	"sync/atomic"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
)

// 埋め込む wasm のモジュール。すべての形式を入れた zumen_abi1.wasm（決定 0221）を、作ってからここに写す。
//
//go:embed zumen_abi1.wasm
var embeddedWasm []byte

// functions は、モジュールが出すはずの関数。
var functions = []string{
	"zm_abi_version",
	"zm_session_new",
	"zm_push",
	"zm_call",
	"zm_take",
	"zm_cancel",
	"zm_session_close",
	"zm_free",
	"zm_alloc",
	"zm_dealloc",
}

// ZumenError は、境界の関数の状態が ok でなかったことを表す。
type ZumenError struct {
	// Operation は、呼んだ境界の関数の名前。
	Operation string

	// Code は、状態コード（Status で始まる定数）。
	Code int32

	// Data は、コアが書いたバイト列（failed の応答）。書かなかったときは nil。
	Data []byte

	// Err は、wasm の実行が止まった理由（取り消し・trap）。境界が状態を返したときは nil。
	Err error
}

// Status は、状態の名前（failed・invalid など）。
func (e *ZumenError) Status() string {
	return StatusName(e.Code)
}

// Response は、書かれた応答。応答の JSON でなければ nil。
func (e *ZumenError) Response() *Response {
	var response Response
	if len(e.Data) == 0 || json.Unmarshal(e.Data, &response) != nil {
		return nil
	}
	return &response
}

func (e *ZumenError) Error() string {
	text := fmt.Sprintf("%s の状態が %s", e.Operation, e.Status())
	if response := e.Response(); response != nil && len(response.Diagnostics) > 0 && response.Diagnostics[0].Code != nil {
		text += "（" + *response.Diagnostics[0].Code + "）"
	}
	if e.Err != nil {
		text += ": " + e.Err.Error()
	}
	return text
}

func (e *ZumenError) Unwrap() error {
	return e.Err
}

// Core は、コンパイルした wasm のモジュールと、それを動かすランタイム。セッションごとに、モジュールのインスタンスを作る。
// ほかのゴルーチンから同時に使える。
type Core struct {
	runtime  wazero.Runtime
	compiled wazero.CompiledModule
}

// New は、埋め込んだ wasm のモジュールを読み込む。
func New(ctx context.Context) (*Core, error) {
	return NewFromWasm(ctx, embeddedWasm)
}

// NewFromWasm は、wasm のモジュールをバイト列から読み込み、関数がそろうことと ABI の大版を確かめる。
func NewFromWasm(ctx context.Context, wasm []byte) (*Core, error) {
	// 取り消した呼び出しは、コンテキストで止める。止めたインスタンスは閉じる
	runtime := wazero.NewRuntimeWithConfig(ctx, wazero.NewRuntimeConfig().WithCloseOnContextDone(true))
	compiled, err := runtime.CompileModule(ctx, wasm)
	if err != nil {
		_ = runtime.Close(ctx)
		return nil, fmt.Errorf("zumen の wasm のモジュールを読めない: %w", err)
	}
	core := &Core{runtime: runtime, compiled: compiled}
	exports := compiled.ExportedFunctions()
	var missing []string
	for _, name := range functions {
		if _, ok := exports[name]; !ok {
			missing = append(missing, name)
		}
	}
	if len(missing) > 0 || len(compiled.ImportedFunctions()) > 0 {
		_ = core.Close(ctx)
		return nil, fmt.Errorf("zumen の wasm のモジュールでない（ない関数: %v、import: %d）", missing, len(compiled.ImportedFunctions()))
	}
	version, err := core.ABIVersion(ctx)
	if err != nil {
		_ = core.Close(ctx)
		return nil, err
	}
	if version != ABIVersion {
		_ = core.Close(ctx)
		return nil, fmt.Errorf("ABI の大版が %d（この包みは %d）", version, ABIVersion)
	}
	return core, nil
}

// Close は、ランタイムと、開いているセッションのインスタンスをすべて閉じる。
func (c *Core) Close(ctx context.Context) error {
	return c.runtime.Close(ctx)
}

func (c *Core) instantiate(ctx context.Context) (api.Module, error) {
	// 名前のないインスタンスにして、いくつでも作れるようにする
	return c.runtime.InstantiateModule(ctx, c.compiled, wazero.NewModuleConfig().WithName(""))
}

// ABIVersion は、モジュールの ABI の大版（zm_abi_version）。
func (c *Core) ABIVersion(ctx context.Context) (uint32, error) {
	module, err := c.instantiate(ctx)
	if err != nil {
		return 0, err
	}
	defer module.Close(ctx)
	results, err := module.ExportedFunction("zm_abi_version").Call(ctx)
	if err != nil {
		return 0, err
	}
	return uint32(results[0]), nil
}

// Open は、設定（session_options）を値で渡して、セッションを作る。nil なら既定の設定。
func (c *Core) Open(ctx context.Context, options *SessionOptions) (*Session, error) {
	if options == nil {
		return c.OpenBytes(ctx, nil)
	}
	data, err := json.Marshal(options)
	if err != nil {
		return nil, err
	}
	return c.OpenBytes(ctx, data)
}

// OpenBytes は、設定を JSON の UTF-8 のバイト列で渡して、セッションを作る。長さが 0 なら既定の設定。
func (c *Core) OpenBytes(ctx context.Context, options []byte) (*Session, error) {
	module, err := c.instantiate(ctx)
	if err != nil {
		return nil, err
	}
	session := &Session{module: module}
	code, err := session.withBytes(ctx, "zm_session_new", options, func(ptr, length uint32) (int32, error) {
		out, err := session.alloc(ctx, 4)
		if err != nil {
			return 0, err
		}
		defer session.dealloc(ctx, out, 4)
		module.Memory().WriteUint32Le(out, 0)
		code, err := session.invoke(ctx, "zm_session_new", uint64(ptr), uint64(length), uint64(out))
		if code == StatusOk {
			session.handle, _ = module.Memory().ReadUint32Le(out)
		}
		return code, err
	})
	if err != nil || code != StatusOk {
		session.closeModule(ctx)
		if err != nil {
			return nil, err
		}
		return nil, &ZumenError{Operation: "zm_session_new", Code: code}
	}
	return session, nil
}

// Session は、コアのセッション（zm_session）と、それを動かす wasm のインスタンス。
//
// 1 つのセッションは、一度に 1 つのゴルーチンだけが使う。Cancel だけは、どのゴルーチンからも呼べる。wasm のモジュールは
// 1 つのスレッドで動くので、Cancel は動いている呼び出しをコンテキストで止め、セッションを閉じる。閉じたセッションへの
// 呼び出しは、invalid の ZumenError になる。
type Session struct {
	mu      sync.Mutex
	module  api.Module
	handle  uint32
	running atomic.Pointer[context.CancelFunc]
}

// IsClosed は、閉じたか。
func (s *Session) IsClosed() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.module == nil
}

func (s *Session) live() api.Module {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.module
}

// closeModule は、インスタンスを閉じる。コアのセッションも、インスタンスと一緒に消える。
func (s *Session) closeModule(ctx context.Context) {
	s.mu.Lock()
	module := s.module
	s.module = nil
	s.mu.Unlock()
	if module != nil {
		_ = module.Close(ctx)
	}
}

// begin は、公開のメソッドの呼び出しを始める。Cancel が止めるコンテキストを返す。終わったら end を呼ぶ。
func (s *Session) begin(ctx context.Context) (context.Context, func()) {
	callCtx, cancel := context.WithCancel(ctx)
	s.running.Store(&cancel)
	return callCtx, func() {
		s.running.Store(nil)
		cancel()
	}
}

// exec は、関数を呼ぶ。実行が止まったら（取り消し・trap）、インスタンスを閉じて ZumenError を返す。
func (s *Session) exec(ctx context.Context, name string, params ...uint64) ([]uint64, error) {
	module := s.live()
	if module == nil {
		return nil, &ZumenError{Operation: name, Code: StatusInvalid}
	}
	results, err := module.ExportedFunction(name).Call(ctx, params...)
	if err != nil {
		s.closeModule(context.WithoutCancel(ctx))
		if ctx.Err() != nil {
			return nil, &ZumenError{Operation: name, Code: StatusFailed, Err: context.Cause(ctx)}
		}
		// wasm では、コアのパニックは trap になる
		return nil, &ZumenError{Operation: name, Code: StatusPanicked, Err: err}
	}
	return results, nil
}

// invoke は、境界の関数を呼び、状態コードを返す。
func (s *Session) invoke(ctx context.Context, name string, params ...uint64) (int32, error) {
	if s.live() == nil {
		return StatusInvalid, nil
	}
	results, err := s.exec(ctx, name, params...)
	if err != nil {
		return 0, err
	}
	if len(results) == 0 {
		return StatusOk, nil
	}
	return int32(uint32(results[0])), nil
}

func (s *Session) alloc(ctx context.Context, length uint32) (uint32, error) {
	results, err := s.exec(ctx, "zm_alloc", uint64(length))
	if err != nil {
		return 0, err
	}
	ptr := uint32(results[0])
	if ptr == 0 {
		return 0, errors.New("wasm のメモリを確保できない")
	}
	return ptr, nil
}

func (s *Session) dealloc(ctx context.Context, ptr, length uint32) {
	if s.live() != nil {
		_, _ = s.exec(ctx, "zm_dealloc", uint64(ptr), uint64(length))
	}
}

// withBytes は、data を wasm のメモリに写して f に渡す。長さ 0 なら番地 0 を渡す。
func (s *Session) withBytes(ctx context.Context, name string, data []byte, f func(ptr, length uint32) (int32, error)) (int32, error) {
	if s.live() == nil {
		return StatusInvalid, nil
	}
	if len(data) == 0 {
		return f(0, 0)
	}
	if uint64(len(data)) > 0xffffffff {
		return 0, fmt.Errorf("%s: 入力が 4 GiB を超える", name)
	}
	ptr, err := s.alloc(ctx, uint32(len(data)))
	if err != nil {
		return 0, err
	}
	defer s.dealloc(ctx, ptr, uint32(len(data)))
	s.live().Memory().Write(ptr, data)
	return f(ptr, uint32(len(data)))
}

// withBuf は、zm_buf（wasm32 では ptr と len の 2 つの u32）を確保して f に渡し、書かれたバイト列を写してから zm_free で返す。
func (s *Session) withBuf(ctx context.Context, f func(buf uint32) (int32, error)) (int32, []byte, error) {
	if s.live() == nil {
		return StatusInvalid, nil, nil
	}
	buf, err := s.alloc(ctx, 8)
	if err != nil {
		return 0, nil, err
	}
	defer s.dealloc(ctx, buf, 8)
	memory := s.live().Memory()
	memory.WriteUint64Le(buf, 0)
	code, err := f(buf)
	if err != nil {
		return 0, nil, err
	}
	var data []byte
	if module := s.live(); module != nil {
		ptr, _ := module.Memory().ReadUint32Le(buf)
		length, _ := module.Memory().ReadUint32Le(buf + 4)
		if ptr != 0 {
			view, ok := module.Memory().Read(ptr, length)
			if !ok {
				return 0, nil, errors.New("コアが返したバッファーが wasm のメモリの外を指す")
			}
			data = bytes.Clone(view)
		}
		if _, err := s.exec(ctx, "zm_free", uint64(buf)); err != nil {
			return 0, nil, err
		}
	}
	if data == nil && code == StatusOk {
		data = []byte{}
	}
	return code, data, nil
}

// Push は、名前のついた入力にバイト列を足す（zm_push）。同じ名前で何回か呼ぶと、後ろに足していく。
func (s *Session) Push(ctx context.Context, name string, data []byte) error {
	return s.PushBytes(ctx, []byte(name), data)
}

// PushBytes は、入力を、名前のバイト列で渡す。
func (s *Session) PushBytes(ctx context.Context, name, data []byte) error {
	ctx, end := s.begin(ctx)
	defer end()
	code, err := s.withBytes(ctx, "zm_push", name, func(namePtr, nameLength uint32) (int32, error) {
		return s.withBytes(ctx, "zm_push", data, func(dataPtr, dataLength uint32) (int32, error) {
			return s.invoke(ctx, "zm_push", uint64(s.handle), uint64(namePtr), uint64(nameLength), uint64(dataPtr), uint64(dataLength))
		})
	})
	if err != nil {
		return err
	}
	if code != StatusOk {
		return &ZumenError{Operation: "zm_push", Code: code}
	}
	return nil
}

// CallBytes は、要求の JSON の UTF-8 を渡し、応答のバイト列を受け取る（zm_call）。
func (s *Session) CallBytes(ctx context.Context, request []byte) ([]byte, error) {
	ctx, end := s.begin(ctx)
	defer end()
	code, data, err := s.withBuf(ctx, func(buf uint32) (int32, error) {
		return s.withBytes(ctx, "zm_call", request, func(ptr, length uint32) (int32, error) {
			return s.invoke(ctx, "zm_call", uint64(s.handle), uint64(ptr), uint64(length), uint64(buf))
		})
	})
	if err != nil {
		return nil, err
	}
	if code != StatusOk || data == nil {
		return nil, &ZumenError{Operation: "zm_call", Code: code, Data: data}
	}
	return data, nil
}

// CallText は、要求を JSON の文字列で渡し、応答の文字列を受け取る。
func (s *Session) CallText(ctx context.Context, request string) (string, error) {
	data, err := s.CallBytes(ctx, []byte(request))
	return string(data), err
}

// Call は、要求の値（要求の型か、JSON にできる値）を渡し、応答を読む。
func (s *Session) Call(ctx context.Context, request any) (*Response, error) {
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

// Take は、直前の Call が作ったバイナリの結果を、番号で取り出す（zm_take）。
func (s *Session) Take(ctx context.Context, blob uint32) ([]byte, error) {
	ctx, end := s.begin(ctx)
	defer end()
	code, data, err := s.withBuf(ctx, func(buf uint32) (int32, error) {
		return s.invoke(ctx, "zm_take", uint64(s.handle), uint64(blob), uint64(buf))
	})
	if err != nil {
		return nil, err
	}
	if code != StatusOk || data == nil {
		return nil, &ZumenError{Operation: "zm_take", Code: code, Data: data}
	}
	return data, nil
}

// Cancel は、動いている呼び出しを止める。止めた呼び出しは failed の ZumenError（Err は context.Canceled）を返し、
// セッションは閉じる。どのゴルーチンからも呼べる。動いている呼び出しがなければ、何もしない。
func (s *Session) Cancel() {
	if cancel := s.running.Load(); cancel != nil {
		(*cancel)()
	}
}

// Close は、セッションを閉じる（zm_session_close）。2 度目からは何もしない。
func (s *Session) Close(ctx context.Context) error {
	if s.live() == nil {
		return nil
	}
	_, err := s.invoke(ctx, "zm_session_close", uint64(s.handle))
	s.closeModule(ctx)
	return err
}

// ResultOf は、応答の op ごとの結果を、その op の結果の型で読む（例: zumen.ResultOf[zumen.OpenResult](response)）。
// 失敗したときは nil。
func ResultOf[T any](response *Response) (*T, error) {
	if response == nil || len(response.Result) == 0 || string(response.Result) == "null" {
		return nil, nil
	}
	var result T
	if err := json.Unmarshal(response.Result, &result); err != nil {
		return nil, err
	}
	return &result, nil
}

// rawJSON は、持っている JSON を返す。空なら null。
func rawJSON(data []byte) []byte {
	if len(data) == 0 {
		return []byte("null")
	}
	return data
}

// asTagged は、目印の欄が値と同じなら、data をその型で読む。違うか読めなければ nil。
func asTagged[T any](data []byte, key, value string) *T {
	var fields map[string]json.RawMessage
	if json.Unmarshal(data, &fields) != nil {
		return nil
	}
	var tag string
	if json.Unmarshal(fields[key], &tag) != nil || tag != value {
		return nil
	}
	var out T
	if json.Unmarshal(data, &out) != nil {
		return nil
	}
	return &out
}
