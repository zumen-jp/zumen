package zumen

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"runtime"
	"slices"
	"strconv"
	"strings"
	"testing"
)

// 適合試験を、Go の包みを通して流す（決定 0006・0226）。
//
// bindings/conformance/cases/*.json を、包みのセッションのメソッドで流し、状態と応答の値が期待どおりで、ハッシュの一覧が
// hashes.txt と一致することを確かめる。シーンの blob は、包みの読み手で読み、応答の数と合うかを確かめる。手順の読み方と
// 応答の確かめ方は、Python・Java の側と同じ規則で書く。リポジトリの根は、ZUMEN_REPOSITORY か、今の場所から上にたどって探す。

// actions は、手順の種類。手順は、どれか 1 つの欄を持つ。
var actions = []string{"session", "push", "call", "call_text", "call_hex", "take"}

var arrayIndex = regexp.MustCompile(`^(0|[1-9][0-9]*)$`)

// missing は、JSON Pointer で引けなかった印。
type missing struct{}

type conformance struct {
	repository string
	failures   []string
	hashes     []string
	scenes     int
}

func (c *conformance) fail(format string, args ...any) {
	c.failures = append(c.failures, fmt.Sprintf(format, args...))
}

// parse は、JSON を読む。数は json.Number のまま持ち、整数と小数を区別する。
func parse(data []byte) (any, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	var value any
	if err := decoder.Decode(&value); err != nil {
		return nil, err
	}
	return value, nil
}

func encode(value any) []byte {
	data, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	return data
}

func decodeHex(text string) ([]byte, error) {
	return hex.DecodeString(strings.Join(strings.Fields(text), ""))
}

// pointer は、JSON Pointer で値を引く。なければ missing。
func pointer(value any, path string) any {
	if path == "" {
		return value
	}
	current := value
	for _, raw := range strings.Split(path[1:], "/") {
		token := strings.ReplaceAll(strings.ReplaceAll(raw, "~1", "/"), "~0", "~")
		switch node := current.(type) {
		case []any:
			if !arrayIndex.MatchString(token) || len(token) > 9 {
				return missing{}
			}
			index, _ := strconv.Atoi(token)
			if index >= len(node) {
				return missing{}
			}
			current = node[index]
		case map[string]any:
			next, ok := node[token]
			if !ok {
				return missing{}
			}
			current = next
		default:
			return missing{}
		}
	}
	return current
}

// same は、JSON の値として等しいか。真偽値と数は別にし、オブジェクトはキーの順によらない。
func same(a, b any) bool {
	switch x := a.(type) {
	case []any:
		y, ok := b.([]any)
		return ok && slices.EqualFunc(x, y, same)
	case map[string]any:
		y, ok := b.(map[string]any)
		if !ok || len(x) != len(y) {
			return false
		}
		for key, value := range x {
			other, ok := y[key]
			if !ok || !same(value, other) {
				return false
			}
		}
		return true
	case json.Number:
		y, ok := b.(json.Number)
		if !ok {
			return false
		}
		if p, err := x.Int64(); err == nil {
			if q, err := y.Int64(); err == nil {
				return p == q
			}
		}
		p, err1 := x.Float64()
		q, err2 := y.Float64()
		return err1 == nil && err2 == nil && p == q
	default:
		return a == b
	}
}

func text(value any) string {
	return string(encode(value))
}

// integer は、JSON の整数。整数でなければ -1。
func integer(value any) int64 {
	if number, ok := value.(json.Number); ok {
		if n, err := number.Int64(); err == nil {
			return n
		}
	}
	return -1
}

// bytesOf は、session の設定か call の要求のバイト列。値は JSON に直し、文字列はそのまま渡す。
func bytesOf(input any, valueKey, textKey string) []byte {
	object, _ := input.(map[string]any)
	if value, ok := object[valueKey]; ok {
		return encode(value)
	}
	if value, ok := object[textKey].(string); ok {
		return []byte(value)
	}
	return nil
}

// readSample は、公開サンプル（決定 0171）を読む。道筋はリポジトリの根から書き、samples/ の中に限る。
func (c *conformance) readSample(file string) ([]byte, error) {
	parts := strings.Split(file, "/")
	valid := len(parts) >= 2 && parts[0] == "samples"
	for _, part := range parts[1:] {
		if part == "" || part == "." || part == ".." || strings.Contains(part, `\`) {
			valid = false
		}
	}
	if !valid {
		return nil, fmt.Errorf("push の file は samples/ の中のファイルにする: %s", file)
	}
	return os.ReadFile(filepath.Join(append([]string{c.repository}, parts...)...))
}

// requestOf は、call・call_text・call_hex の手順の、要求のバイト列。
func requestOf(action string, input any) ([]byte, error) {
	switch action {
	case "call_text":
		value, _ := input.(string)
		return []byte(value), nil
	case "call_hex":
		value, _ := input.(string)
		return decodeHex(value)
	default:
		return encode(input), nil
	}
}

// attempt は、境界の関数の結果を、状態の名前と書かれたバイト列にする。
func attempt(data []byte, err error) (string, []byte, error) {
	if err == nil {
		return "ok", data, nil
	}
	var zumenError *ZumenError
	if errors.As(err, &zumenError) && zumenError.Err == nil {
		return zumenError.Status(), zumenError.Data, nil
	}
	return "", nil, err
}

// checkResponse は、応答の値を、手順の expect と absent で確かめる。
func (c *conformance) checkResponse(at string, step map[string]any, response any) {
	if expect, ok := step["expect"].(map[string]any); ok {
		for path, value := range expect {
			found := pointer(response, path)
			if _, ok := found.(missing); ok {
				c.fail("%s: %s がない", at, path)
			} else if !same(found, value) {
				c.fail("%s: %s が %s（期待は %s）", at, path, text(found), text(value))
			}
		}
	}
	if absent, ok := step["absent"].([]any); ok {
		for _, raw := range absent {
			path, _ := raw.(string)
			if _, ok := pointer(response, path).(missing); !ok {
				c.fail("%s: %s があってはいけない", at, path)
			}
		}
	}
}

// checkScene は、シーンの blob を読み手で読み、応答の数と合うかを確かめる。
func (c *conformance) checkScene(at string, blob []byte, result map[string]any) {
	scene, err := ReadScene(blob)
	if err != nil {
		c.fail("%s: シーンを読めない: %v", at, err)
		return
	}
	counts, _ := result["counts"].(map[string]any)
	chunks := integer(counts["chunks"])
	rows := func(name string) int64 {
		n, err := scene.Rows(name)
		if err != nil {
			c.fail("%s: %v", at, err)
		}
		return int64(n)
	}
	checks := []struct {
		name          string
		got, expected int64
	}{
		{"大版", int64(scene.Major), int64(SceneMajor)},
		{"区画の数", int64(scene.Sections), int64(len(SceneSections))},
		{"塊の数", int64(scene.Chunks), chunks},
		{"chunk_origins の行", rows("chunk_origins"), chunks},
		{"text_xy の行", rows("text_xy"), integer(counts["texts"])},
		{"point_xy の行", rows("point_xy"), integer(counts["points"])},
		{"glyph_xy の行", rows("glyph_xy"), integer(counts["glyphs"])},
		{"line_xy と face_xy の行", rows("line_xy") + rows("face_xy"), integer(counts["vertices"])},
	}
	// 文字の文字列は、strings の区画から引ける
	textStrings, err := scene.U32("text_strings")
	if err != nil {
		c.fail("%s: %v", at, err)
	}
	for row := 0; row+1 < len(textStrings); row += 2 {
		if _, err := scene.Text(textStrings[row], textStrings[row+1]); err != nil {
			c.fail("%s: text_strings の行 %d を引けない: %v", at, row/2, err)
		}
	}
	for _, check := range checks {
		if check.got != check.expected {
			c.fail("%s: シーンの%sが %d（期待は %d）", at, check.name, check.got, check.expected)
		}
	}
}

func (c *conformance) runCase(ctx context.Context, core *Core, name string, steps []any) error {
	var session *Session
	defer func() {
		if session != nil {
			_ = session.Close(ctx)
		}
	}()
	// 直前の応答が返したシーンの blob の番号と結果
	var lastScene map[string]any
	lastBlob := int64(-1)
	for index, raw := range steps {
		at := fmt.Sprintf("%s の手順 %d", name, index)
		step, _ := raw.(map[string]any)
		action := ""
		for _, candidate := range actions {
			if _, ok := step[candidate]; ok {
				action = candidate
				break
			}
		}
		if action == "" {
			c.fail("%s: 手順の種類がない", at)
			continue
		}
		expected := "ok"
		if status, ok := step["status"].(string); ok {
			expected = status
		}
		input := step[action]
		if action != "session" && session == nil {
			opened, err := core.Open(ctx, nil)
			if err != nil {
				return err
			}
			session = opened
		}
		var (
			status string
			data   []byte
			kind   string
			err    error
		)
		switch action {
		case "session":
			if session != nil {
				_ = session.Close(ctx)
			}
			var opened *Session
			opened, err = core.OpenBytes(ctx, bytesOf(input, "options", "options_text"))
			status, _, err = attempt(nil, err)
			if err != nil {
				return err
			}
			if status == "ok" {
				session = opened
			} else {
				// 作れなかったら、閉じたセッションで続ける。閉じたセッションへの呼び出しは invalid になる
				if session, err = core.Open(ctx, nil); err != nil {
					return err
				}
				_ = session.Close(ctx)
			}
		case "push":
			push, _ := input.(map[string]any)
			var pushName, content []byte
			if value, ok := push["name_hex"].(string); ok {
				if pushName, err = decodeHex(value); err != nil {
					return err
				}
			} else {
				value, _ := push["name"].(string)
				pushName = []byte(value)
			}
			if value, ok := push["hex"].(string); ok {
				content, err = decodeHex(value)
			} else if value, ok := push["file"].(string); ok {
				content, err = c.readSample(value)
			} else {
				value, _ := push["text"].(string)
				content = []byte(value)
			}
			if err != nil {
				return err
			}
			status, _, err = attempt(nil, session.PushBytes(ctx, pushName, content))
		case "take":
			blob := integer(input)
			status, data, err = attempt(session.Take(ctx, uint32(blob)))
			if data != nil {
				kind = "blob"
			}
			if status == "ok" && lastScene != nil && lastBlob == blob {
				c.checkScene(at, data, lastScene)
				c.scenes++
			}
		default:
			var request []byte
			if request, err = requestOf(action, input); err != nil {
				return err
			}
			status, data, err = attempt(session.CallBytes(ctx, request))
			if data != nil {
				kind = "response"
			}
		}
		if err != nil {
			return fmt.Errorf("%s: %w", at, err)
		}
		if status != expected {
			c.fail("%s: 状態が %s（期待は %s）", at, status, expected)
		}
		if kind == "" {
			continue
		}
		digest := sha256.Sum256(data)
		c.hashes = append(c.hashes, fmt.Sprintf("%s %d %s %s", name, index, kind, hex.EncodeToString(digest[:])))
		if kind != "response" {
			continue
		}
		response, err := parse(data)
		if err != nil {
			c.fail("%s: 応答が JSON でない: %v", at, err)
			continue
		}
		c.checkResponse(at, step, response)
		lastScene, lastBlob = nil, -1
		object, _ := response.(map[string]any)
		result, _ := object["result"].(map[string]any)
		blobs, _ := object["blobs"].([]any)
		if result == nil {
			continue
		}
		for _, entry := range blobs {
			blob, _ := entry.(map[string]any)
			if blob["kind"] == "scene" {
				lastScene, lastBlob = result, integer(blob["id"])
				break
			}
		}
	}
	return nil
}

// compareHashes は、ハッシュの一覧が hashes.txt と一致するかを確かめる。
func (c *conformance) compareHashes() error {
	data, err := os.ReadFile(filepath.Join(c.repository, "bindings", "conformance", "hashes.txt"))
	if err != nil {
		return err
	}
	var expected []string
	for _, line := range strings.Split(string(data), "\n") {
		if line != "" && !strings.HasPrefix(line, "#") {
			expected = append(expected, line)
		}
	}
	if slices.Equal(c.hashes, expected) {
		return nil
	}
	message := "ハッシュの一覧が hashes.txt と合わない:"
	for _, line := range expected {
		if !slices.Contains(c.hashes, line) {
			message += "\n- " + line
		}
	}
	for _, line := range c.hashes {
		if !slices.Contains(expected, line) {
			message += "\n+ " + line
		}
	}
	c.fail("%s", message)
	return nil
}

// findRepository は、ZUMEN_REPOSITORY か、今の場所から上にたどって、bindings/conformance のあるフォルダーを根にする。
func findRepository(t *testing.T) string {
	if root := os.Getenv("ZUMEN_REPOSITORY"); root != "" {
		return root
	}
	current, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for {
		if _, err := os.Stat(filepath.Join(current, "bindings", "conformance", "hashes.txt")); err == nil {
			return current
		}
		parent := filepath.Dir(current)
		if parent == current {
			t.Fatal("リポジトリの根が見つからない。ZUMEN_REPOSITORY で渡す")
		}
		current = parent
	}
}

func openCore(t *testing.T) *Core {
	t.Helper()
	ctx := context.Background()
	core, err := New(ctx)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = core.Close(ctx) })
	return core
}

func TestConformance(t *testing.T) {
	ctx := context.Background()
	core := openCore(t)
	c := &conformance{repository: findRepository(t)}
	files, err := filepath.Glob(filepath.Join(c.repository, "bindings", "conformance", "cases", "*.json"))
	if err != nil {
		t.Fatal(err)
	}
	slices.Sort(files)
	for _, file := range files {
		data, err := os.ReadFile(file)
		if err != nil {
			t.Fatal(err)
		}
		value, err := parse(data)
		if err != nil {
			t.Fatalf("%s: %v", file, err)
		}
		object, _ := value.(map[string]any)
		steps, _ := object["steps"].([]any)
		name := strings.TrimSuffix(filepath.Base(file), ".json")
		if err := c.runCase(ctx, core, name, steps); err != nil {
			t.Fatalf("%s: %v", name, err)
		}
	}
	if err := c.compareHashes(); err != nil {
		t.Fatal(err)
	}
	summary := fmt.Sprintf("包みを通した試験の組 %d 件、ハッシュ %d 件、読んだシーン %d 件", len(files), len(c.hashes), c.scenes)
	t.Log(summary)
	if path := os.Getenv("GITHUB_STEP_SUMMARY"); path != "" {
		if file, err := os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644); err == nil {
			_, _ = fmt.Fprintf(file, "### Go の包みの適合試験（%s）\n\n%s\n", strings.TrimPrefix(runtime.Version(), "go"), summary)
			_ = file.Close()
		}
	}
	if c.scenes == 0 {
		c.fail("シーンの blob を 1 つも読んでいない")
	}
	for _, failure := range c.failures {
		t.Error(failure)
	}
}

// TestTypedCalls は、型を渡す call と、失敗した call と、閉じたあとの呼び出しを確かめる。
func TestTypedCalls(t *testing.T) {
	ctx := context.Background()
	core := openCore(t)
	session, err := core.Open(ctx, &SessionOptions{V: new(SchemaMajor)})
	if err != nil {
		t.Fatal(err)
	}
	response, err := session.Call(ctx, CapabilitiesRequest{V: new(SchemaMajor)})
	if err != nil {
		t.Fatal(err)
	}
	result, err := ResultOf[CapabilitiesResult](response)
	if err != nil || response.V == nil || *response.V != SchemaMajor || result == nil || result.Package == nil {
		t.Errorf("call の応答が capabilities の形でない: %+v（%v）", response, err)
	}
	_, err = session.Call(ctx, OpenRequest{V: new(SchemaMajor), Input: new("none.dxf")})
	var failed *ZumenError
	if !errors.As(err, &failed) || failed.Status() != "failed" || failed.Response() == nil || len(failed.Response().Diagnostics) == 0 {
		t.Errorf("ない入力を開く call の失敗が、診断のある failed でない: %v", err)
	}
	// ほかのゴルーチンから Cancel を呼べる（動いている呼び出しがなければ、何もしない）
	done := make(chan struct{})
	go func() {
		session.Cancel()
		close(done)
	}()
	<-done
	if session.IsClosed() {
		t.Error("動いている呼び出しのない Cancel で、セッションが閉じた")
	}
	if err := session.Close(ctx); err != nil {
		t.Fatal(err)
	}
	_, err = session.CallBytes(ctx, encode(CapabilitiesRequest{V: new(SchemaMajor)}))
	if !session.IsClosed() || !errors.As(err, &failed) || failed.Status() != "invalid" {
		t.Errorf("閉じたあとの call が %v（期待は invalid）", err)
	}
}

// TestCancel は、取り消したコンテキストの呼び出しが failed で止まり、セッションが閉じることを確かめる。
func TestCancel(t *testing.T) {
	ctx := context.Background()
	core := openCore(t)
	session, err := core.Open(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	canceled, cancel := context.WithCancel(ctx)
	cancel()
	_, err = session.CallBytes(canceled, encode(CapabilitiesRequest{V: new(SchemaMajor)}))
	var failed *ZumenError
	if !errors.As(err, &failed) || failed.Status() != "failed" || !errors.Is(err, context.Canceled) {
		t.Errorf("取り消した呼び出しが %v（期待は context.Canceled の failed）", err)
	}
	if !session.IsClosed() {
		t.Error("取り消したあと、セッションが閉じていない")
	}
	// ほかのセッションは、続けて使える
	other, err := core.Open(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer other.Close(ctx)
	if _, err := other.CallText(ctx, `{"v":1,"op":"capabilities"}`); err != nil {
		t.Errorf("取り消したあとの、ほかのセッションの call が失敗した: %v", err)
	}
}
