# 3D のカーネルの候補の比較

[#414](https://github.com/zumen-jp/zumen/issues/414) の試作です。`zumen-geom3d` の合成の形の組（`zumen_geom3d::cases`）を、カーネルの候補ごとに評価し、体積・囲む箱・閉じているか・平面での切り口・出力の指紋（座標のビット）を比べます。試料での速さと失敗の割合は、[#5](https://github.com/zumen-jp/zumen/issues/5) で測ります。結果の判断は [決定 0222](../../docs/decisions/0222-geom3d-types-and-kernel-trait.md) にあります。

コアのワークスペースには入れていません。候補は、選ぶまで workspace の依存に入れません（決定 0018）。

## 候補

| feature | 候補 | ブーリアン |
| --- | --- | --- |
| `own` | 自前と [i_overlay](https://crates.io/crates/i_overlay)（MIT OR Apache-2.0） | 断面に垂直に押し出した形から、同じ向きで厚さを覆う押し出しを引くときだけ、断面の 2D のブーリアンにする |
| `manifold` | [manifold-rust](https://crates.io/crates/manifold-rust) 0.19.1（Apache-2.0）。C++ の Manifold の Rust への移植 | メッシュのブーリアン |
| `csgrs` | [csgrs](https://crates.io/crates/csgrs) 0.20.1（MIT）。BSP の木 | メッシュのブーリアン |

- ブーリアンのない形（押し出し・回転・円の断面の掃引・面の集合・三角形の集合・写像・組）は、どの候補でも `src/primitives.rs` の同じコードで三角形にし、ブーリアンだけを候補に任せます。半空間は、manifold-rust は `trim_by_plane`、csgrs は大きな箱とのブーリアンにします。
- csgrs が依存する `core2` は、crates.io のすべての版が取り下げられていて、新しく解決できません。`Cargo.toml` で上流のリポジトリの 0.4.0 を当てています。
- C++ の Manifold を呼ぶ [manifold-csg](https://crates.io/crates/manifold-csg) は、作るときに C++ の Manifold を git で取得し、cmake と C++ のコンパイラーで作ります。wasm32-unknown-unknown では、libc++ の見出しを持つ LLVM の clang と wasm-cxx-shim の取得が要ります（`manifold-csg-sys` 3.5.105 の `build.rs`）。決定 0018 の「作るのに C・C++ の道具が要らない」に合わないので、比べていません。

## 流し方

```bash
cargo build -q --release
./run.sh ./target/release/compare
rustup target add wasm32-wasip1
cargo build -q --release --bin compare --target wasm32-wasip1
./run.sh node --no-warnings wasi.mjs target/wasm32-wasip1/release/compare.wasm
./size.sh
```

- `compare [候補] [組]` は、組ごとに 1 行を書きます。`status` は、期待どおりなら `ok`、閉じていないか体積・箱が違えば `wrong`、評価しない形なら `unsupported`、正しくない形を断れば `rejected` です。`section` は、箱の高さの中ほどを水平の平面（`zumen_geom3d::MeshSection`）で切った、輪の数・開いた鎖の数・面積です。
- `run.sh` は、候補と組ごとに別のプロセスで流し、60 秒で打ち切ります。wasm はパニックで止まるので、組ごとに分けます。
- `wasi.mjs` は、wasm32-wasip1 に作った `compare` を Node.js の WASI で流します。
- `size.sh` は、候補を 1 つずつ入れた wasm32-unknown-unknown の cdylib を作り、ブーリアンのない基準（`none`）との差を出します。

## 結果（2026-10-31）

26 組（閉じた立体 21・閉じていないか厚さのない形とのブーリアン 2・正しくない形 3）。手元は macOS arm64、Rust 1.99.1、Node.js 24。

| | 自前（i_overlay） | manifold-rust | csgrs |
| --- | ---: | ---: | ---: |
| 立体 21 のうち期待どおり | 15 | 21 | 15 |
| 立体で、閉じていない出力 | 0 | 0 | 6 |
| 評価しない（ブーリアン） | 6 | 0 | 0 |
| 閉じていない・厚さのない形とのブーリアン | 評価しない | 失敗を返す 1・元の形を返す 1 | 閉じていない出力 2 |
| パニック・止まらない | 0 | 0 | 0 |
| ネイティブ（arm64・x86_64）と wasm32 の指紋 | 一致 | 一致 | 一致 |
| wasm の増分（そのまま / brotli、バイト） | 213,304 / 53,328 | 1,033,397 / 243,854 | 62,779 / 13,771 |
| 依存の crate の数 | 10 | 15 | 79 |
| 依存のライセンス（`deny.toml`） | 通る | 通る | 通る |
| 依存の検査のほかの指摘 | なし | なし | 維持されていない `paste`（nalgebra の下）、取り下げた `core2` |

- csgrs は、ブーリアンの体積は合うが、BSP で分けた面に T 字の継ぎ目が残り、閉じたメッシュにならない。切り口は輪にならず、面積も合わない（開口のある壁で 0.56 のところ 0）。閉じていない面を引くと、閉じていない形を返す。
- manifold-rust は、同じ平面にある面・1e-9 だけずれた面・半空間を含め、立体のすべてで、閉じたメッシュと期待する体積・切り口になった。閉じていない入力は `NotManifold` で断る。
- 自前は、扱える形（床の開口・円の穴）では manifold-rust と同じ体積と切り口になったが、壁の開口（押し出しの向きが違う）・和・積・半空間は扱えない。
- 正しくない形（長さ 0・有限でない値・点の番号の範囲の外）は、どの候補でも、ブーリアンの前に自前のコードで断った。候補そのものに渡したときの振る舞いは、試していない。
- 指紋は、macOS arm64 と x86_64（Rosetta 2）のネイティブと、wasm32-wasip1（Node.js の WASI）で、78 行すべてが一致した。Linux と Windows では、まだ流していない。
- コードの中の数（試験を除く）: manifold-rust は約 4.6 万行で、パニックしうる所（`unwrap`・`expect`・`panic!`・`assert!` など）が 646、std の三角関数などが 42、std の `HashMap` を使うファイルが 6（反復の順が実行ごとに変わりうる。3 回流した指紋は同じだった）。csgrs は約 1.4 万行で、それぞれ 95・121。i_overlay は約 1.7 万行で、20・3、`unsafe` が 49。
