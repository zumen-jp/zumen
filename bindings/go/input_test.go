package zumen

import (
	"bytes"
	"context"
	"errors"
	"testing"
)

// 同じ wasm の新しい関数の名前だけを変え、旧モジュールへの互換経路も実際に動かす。
func TestInputOwnership(t *testing.T) {
	const mib = 1024 * 1024
	ctx := context.Background()
	for _, mode := range []string{"owned", "legacy"} {
		t.Run(mode, func(t *testing.T) {
			wasm := embeddedWasm
			if mode == "legacy" {
				// 同じ長さの名前にして、exports と名前の節以外の配置を変えない
				wasm = bytes.ReplaceAll(wasm, []byte("zm_push_owned"), []byte("xx_push_owned"))
			}
			core, err := NewFromWasm(ctx, wasm)
			if err != nil {
				t.Fatal(err)
			}
			defer core.Close(ctx)
			open := func() *Session {
				t.Helper()
				session, err := core.Open(ctx, nil)
				if err != nil {
					t.Fatal(err)
				}
				t.Cleanup(func() { _ = session.Close(ctx) })
				return session
			}
			session := open()
			if present := session.live().ExportedFunction("zm_push_owned") != nil; present != (mode == "owned") {
				t.Fatal("検査する関数の有無が合わない")
			}
			sample := []byte("0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n1\n21\n2\n0\nENDSEC\n0\nEOF\n")
			for _, data := range [][]byte{nil, sample[:20], sample[20:]} {
				if err := session.Push(ctx, "a.dxf", data); err != nil {
					t.Fatal(err)
				}
			}
			clear(sample)
			response, err := session.Call(ctx, map[string]any{"v": 1, "op": "open", "input": "a.dxf"})
			if err != nil {
				t.Fatal(err)
			}
			opened, err := ResultOf[OpenResult](response)
			if err != nil || opened == nil || opened.Format == nil || *opened.Format != "dxf" {
				t.Fatalf("追記した入力を開けない: %v", err)
			}
			invalid := func(err error) {
				t.Helper()
				var got *ZumenError
				if !errors.As(err, &got) || got.Code != StatusInvalid || got.Operation != "zm_push" {
					t.Fatalf("無効な引数の状態が違う: %v", err)
				}
			}
			large := make([]byte, 4*mib)
			invalid(session.Push(ctx, "../bad", large))
			memory := session.live().Memory()
			afterFirst := memory.Size()
			for range 16 {
				invalid(session.Push(ctx, "../bad", large))
			}
			if memory.Size() > afterFirst+mib {
				t.Fatal("無効な名前で入力の領域が残る")
			}
			if err := session.Close(ctx); err != nil {
				t.Fatal(err)
			}
			invalid(session.Push(ctx, "closed", large))

			measured := open()
			memory = measured.live().Memory()
			before := memory.Size()
			if err := measured.Push(ctx, "large", make([]byte, 32*mib)); err != nil {
				t.Fatal(err)
			}
			delta := memory.Size() - before
			t.Logf("%s: 32 MiB の入力、wasm のメモリの増加 %d バイト", mode, delta)
			if (mode == "owned" && delta > 40*mib) || (mode == "legacy" && delta < 56*mib) {
				t.Fatal("メモリの削減が測定の範囲を外れた")
			}
			if err := measured.Close(ctx); err != nil {
				t.Fatal(err)
			}

			canceled := open()
			cancelCtx, cancel := context.WithCancel(ctx)
			cancel()
			var stopped *ZumenError
			if err := canceled.Push(cancelCtx, "a", large); !errors.As(err, &stopped) || stopped.Code != StatusFailed || !errors.Is(err, context.Canceled) {
				t.Fatalf("中断した入力の状態が違う: %v", err)
			}
			if !canceled.IsClosed() {
				t.Fatal("中断したインスタンスが閉じられていない")
			}
		})
	}
}
