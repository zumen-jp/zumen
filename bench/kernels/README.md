# 3D のカーネルの候補の比較

[#414](https://github.com/zumen-jp/zumen/issues/414) の試作です。`zumen-geom3d` の合成の形の組（`zumen_geom3d::cases`）を、カーネルの候補ごとに評価し、体積・囲む箱・閉じているか・平面での切り口・出力の指紋（座標のビット）を比べます。結果の判断は [決定 0222](../../docs/decisions/0222-geom3d-types-and-kernel-trait.md) にあります。

[#5](https://github.com/zumen-jp/zumen/issues/5) では、公開サンプルの IFC（`samples/ifc`）で、速さと失敗の割合を測りました（[公開サンプルの IFC で測る](#公開サンプルの-ifc-で測る)）。結果の判断は [決定 0257](../../docs/decisions/0257-kernel-and-building-items-from-samples.md) にあります。

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
- ブーリアンの前に、どの候補でも、点を `Tolerance::length` の格子にそろえます（`src/kernels.rs`）。公開サンプルの IFC で要った手当てです（下の「公開サンプルの IFC で測る」）。次の合成の組の結果は、この手当ての前に測りました。手当ての後に Windows x64 で流し直しても、組ごとの状態は同じでした（2026-11-03）。

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

## 公開サンプルの IFC で測る

[#5](https://github.com/zumen-jp/zumen/issues/5) で、`samples/ifc` の公開サンプル 12 件（[#412](https://github.com/zumen-jp/zumen/issues/412)）を、候補ごとに測りました。国内のソフトの書き出し 9 件（[#513](https://github.com/zumen-jp/zumen/issues/513) の B01〜B09）は、公開の許諾がないので Git に入れず、手元で同じ手順で測った数だけを書きます。

- `samples [候補 | lift] <IFC>...` は、IFC を `zumen-ifc` で読み、建物モデルに写し（lift）、要素ごとの形状表現 `Body` を `zumen_geom3d::Shape` に写して（`src/ifc.rs`）、候補で評価する。開口（`IfcRelVoidsElement`）は、開口の `Body` を、なければ `Reference` を、母体の配置に対する相対の配置で写し、母体から引く。母体が部品の組（層ごとの壁など）なら、部品ごとに引く。
- 行には、要素ごとの状態・体積・三角形の数・時間・指紋・高さの中ほどの切り口を書く。状態は、閉じたメッシュなら `ok`、部品ごとに閉じた組なら `parts`、閉じた部分どうしが辺で接する形なら `touching`、閉じていても体積のない形（両面の面）なら `flat`、閉じていなければ `open`、評価しない形なら `unsupported:種類`、正しくない形なら `invalid:理由`、カーネルが失敗すれば `failed:理由`、パニックなら `panic` になる。
- `#` で始まる行は、試料と候補ごとの要約で、読む・写す・形に写す時間と、メモリの頂点（割り当てを数える）を書く。`lift` は評価せず、読んで写すまでを測る。
- `samples.sh` は、試料と候補ごとに別のプロセスで流し、15 分で打ち切る。`wasi-samples.mjs` は、wasm32-wasip1 に作った `samples` を Node.js の WASI で流し、`samples/` を `/samples` に見せる。
- `ifcopenshell_volumes.py` は、`ok`・`parts`・`touching` の要素の体積を、IfcOpenShell（試験の道具で、依存にしない）が開口を引いて作った形の体積と照らす。

```bash
cargo build -q --release --bin samples
./samples.sh <出力のフォルダー> ./target/release/samples
cargo build -q --release --bin samples --target wasm32-wasip1
./samples.sh <出力のフォルダー> node --no-warnings wasi-samples.mjs target/wasm32-wasip1/release/samples.wasm
python -I ifcopenshell_volumes.py <出力のフォルダー> manifold-rust
```

### 写す形の項目

`src/ifc.rs` は、次の項目を写します。公開サンプルの 12 件の `Body` と開口の形は、すべて写せました。

| 種類 | 項目 |
| --- | --- |
| 立体・面 | `IfcExtrudedAreaSolid`・`IfcFacetedBrep`・`IfcShellBasedSurfaceModel`・`IfcFaceBasedSurfaceModel`・`IfcPolygonalFaceSet`・`IfcTriangulatedFaceSet` |
| ブーリアン | `IfcBooleanResult`・`IfcBooleanClippingResult`・`IfcHalfSpaceSolid`・`IfcBoxedHalfSpace`・`IfcPolygonalBoundedHalfSpace` |
| 写像 | `IfcMappedItem`（`IfcCartesianTransformationOperator3D`・`IfcCartesianTransformationOperator3DnonUniform`） |
| 断面 | `IfcRectangleProfileDef`・`IfcCircleProfileDef`・`IfcArbitraryClosedProfileDef`・`IfcArbitraryProfileDefWithVoids` |
| 曲線 | `IfcPolyline`・`IfcIndexedPolyCurve`（`IfcLineIndex`・`IfcArcIndex`）・`IfcCompositeCurve`・`IfcTrimmedCurve`（`IfcLine`・`IfcCircle`）・`IfcCircle` |

### 試料で要った手当て

どれも、どの候補にも同じに当てます。

1. 断面と面を三角形にする手順（earcut）は、同じ直線の上の点を落とし、穴の頂点を通る辺を作るので、T 字の継ぎ目が残る。逆向きの辺がない辺だけを、その上の点で分ける（`src/primitives.rs`）。分けないと、押し出しのメッシュが閉じず、manifold-rust が `NotManifold` で断った。
2. 面積のない面（点が 1 直線に並ぶ面など）は、要素を断らずに飛ばす。国内の書き出しの 214 要素（多くは設備の継手）にあった。飛ばすと、132 要素が閉じた形（`ok` か `parts`）になった。
3. Archicad 28 の書き出し（GNI）は、開口の形を `Body` でなく `Reference` に書く。`Body` がなければ `Reference` を使う。
4. 層ごとの壁など、閉じた部品の組の母体は、部品ごとに開口を引く。組を 1 つのメッシュにして引くと、閉じた形にならない。
5. ブーリアンの前に、点を `Tolerance::length`（1e-9 m）の格子にそろえる（`src/kernels.rs`）。BIM Whale の TallBuilding の壁（インスタンス `#2401`）は、面が開口の面から 1.6e-13 mm ずれていて、3 つの開口を順に引くと、manifold-rust が 1 つの開口のうち 915 × 200 × 305 mm を引かなかった。逆の順に引く・先に開口の和を取る・格子にそろえる、のどれでも正しい体積になる。そろえると、csgrs のパニックも 104 から 5 に減った。

### 結果（2026-11-03）

Windows 11 x64（AMD Ryzen 5 3600XT、メモリ 32 GB）、Rust 1.99.1、Node.js 24.21.0 で測りました。許容差は、弦の誤差 1 mm、同じ点とみなす距離 1e-9 m です（ファイルの長さの単位に換算）。時間はネイティブで、試料と候補ごとに別のプロセスで流した値です。

公開サンプルの結果は、次のとおりです（自前の列は、評価しない種類と件数）。

| 試料 | 要素 | 読む・写す（ms） | 頂点（MB） | 形（manifold-rust） | 開口のある要素 | 開口を引いた結果: manifold-rust / csgrs / 自前 | 形・開口の時間（ms） |
| --- | ---: | ---: | ---: | --- | ---: | --- | ---: |
| `bim-whale/LargeBuilding` | 122 | 26・41 | 17.5 | ok 67・open 11・parts 44 | 15 | ok 15 / open 15 / boolean 10・half_space 5 | 43・31 |
| `bim-whale/TallBuilding` | 58 | 18・21 | 9.0 | ok 36・parts 22 | 12 | ok 12 / open 12 / half_space 12 | 9・11 |
| `gni/model_125` | 416 | 64・86 | 39.5 | ok 177・open 134・parts 105 | 57 | ok 24・parts 33 / open 53・panic 4 / boolean 57 | 122・235 |
| `gni/model_18` | 194 | 119・58 | 20.0 | ok 104・open 7・parts 66・touching 17 | 17 | ok 17 / open 17 / boolean 15・half_space 2 | 71・62 |
| `gni/model_9` | 148 | 10・15 | 6.8 | ok 96・parts 44・touching 8 | 12 | ok 12 / open 12 / boolean 12 | 36・56 |
| `nakano/nakano_20F_Restaurant` | 62 | 10・9 | 4.4 | ok 61・open 1 | 0 | - | 26・0 |
| `nakano/nakano_6F_chapel` | 208 | 7・17 | 4.2 | flat 196・ok 3・open 9 | 0 | - | 3・0 |
| `nakano/nakano_ConcertHall` | 80 | 3・3 | 1.7 | flat 66・ok 14 | 0 | - | 1・0 |
| `nakano/nakano_Outside` | 37 | 2・2 | 1.2 | flat 8・ok 24・touching 5 | 0 | - | 1・0 |
| `nakano/nakano_entrance` | 188 | 12・16 | 7.0 | flat 17・ok 143・open 4・touching 24 | 0 | - | 6・0 |
| `rinya/wood-office-architecture` | 342 | 44・65 | 21.8 | ok 304・parts 18・touching 20 | 58 | ok 58 / ok 4・open 53・panic 1 / boolean 58 | 54・147 |
| `text2bim/sample_model` | 19 | 8・8 | 3.7 | ok 9・open 7・touching 3 | 0 | - | 1・0 |

- manifold-rust は、開口のある 171 要素のすべてで、開口を引いた結果が閉じた形（`ok` か `parts`）になった。パニックと失敗はなかった。
- csgrs は、開口を引いた結果が 4 要素しか閉じず（BSP の T 字の継ぎ目）、model_125 の 4 要素と林野庁の 1 要素でパニックした。wasm32 では、パニックで試料の全体が止まった。
- 自前は、開口のある要素のどれも評価しない（壁の開口は押し出しの向きが母体と違い、半空間での切断もある）。
- `open` と `flat` は、面の集合で書いた家具・設備・中野区の地形などに出た。ブーリアンには渡さない。
- ネイティブで 2 度流した 5,622 行は、すべて同じだった。wasm32-wasip1 の 4,864 行（manifold-rust の 1,874 行を含む）は、ネイティブと座標のビットまで同じだった。残りの 758 行は、wasm32 でパニックして止まった csgrs の 2 組の分で、比べられなかった。
- wasm32 の時間は、読む・写すがネイティブの約 2.2 倍、形と開口が約 1.5 倍だった（開口を引く時間は、1 件で 0.33 秒以下）。メモリの頂点は、ネイティブの約 0.7 倍だった（ポインターが 32 ビット）。

IfcOpenShell 0.9.0.post1 の既定の設定（開口を引く）と、manifold-rust の `ok`・`parts`・`touching` の 1,414 要素の体積を照らしました。

| 相対の差 | 要素 |
| --- | ---: |
| 1e-6 以下 | 1,278 |
| 1e-6 を超え 1e-4 以下 | 76 |
| 1e-4 を超え 1e-3 以下 | 1 |
| 1e-3 を超える | 3 |
| 体積が 1e-5 m³ 未満（比べない） | 56 |

- 1e-3 を超える 3 要素は、GNI の model_125 の床で、重なる部品の体積を、IfcOpenShell は和にしてから求め、試作は部品ごとに足すので違う。
- 1e-6 を超え 1e-3 以下の 77 要素のうち 76 は、中野区の entrance の要素である（ほかの 1 つは model_125 の壁で、2.3e-6）。形は平面直角座標（原点から約 3.6 万 m）のまま書かれていて、面は平ら（ずれ 1e-11 m 以下）なので、体積の式の桁落ちと推定する。

国内のソフトの書き出し（Git に入れていない）の、manifold-rust での結果は、次のとおりです。

| ID | 書き出したソフト | 大きさ（MB） | 読む・写す（ms） | 読む・写すの頂点（MB） | 形 | 開口のある要素 | 開口を引いた結果 |
| --- | --- | ---: | ---: | ---: | --- | ---: | --- |
| B01 | CADEWA Real2011 | 1.1 | 25・131 | 14・15 | ok 132・open 37・touching 1 | 0 | - |
| B02 | Archicad 20 日本語版 | 2.1 | 57・80 | 24・28 | ok 758・open 9・touching 3 | 30 | ok 30 |
| B03 | Archicad 20 日本語版 | 0.7 | 14・36 | 8・8 | ok 150・open 3 | 8 | ok 8 |
| B04 | ArchiCAD 15 日本語版 | 6.5 | 255・246 | 83・87 | ok 236・open 119・parts 14・touching 8 | 28 | ok 16・touching 12 |
| B05 | GLOOBE Architect 2024 | 36.2 | 1,144・1,275 | 457・475 | ok 1,576・parts 180・touching 197・open 11・flat 4 | 161 | ok 99・failed 3（ほかの 59 は柱の断面を写していない） |
| B06 | Rebro2025 | 48.7 | 939・1,866 | 507・635 | ok 2,015・parts 652・open 142・touching 12 | 0 | - |
| B07 | CADWe'll Linx V6 | 57.2 | 1,492・2,439 | 739・867 | ok 2,421・parts 438・open 303 | 0 | - |
| B08 | CADEWA Smart V5.0 | 63.2 | 1,237・2,182 | 603・787 | ok 1,950・parts 217・open 513・touching 35 | 0 | - |
| B09 | FILDER CeeD | 245.3 | 5,618・7,676 | 2,867・2,996 | ok 2,096・parts 97・open 488・touching 52・flat 15 | 0 | - |

- 9 件とも、パニックと止まりはなかった。形を写せた開口のある 168 要素のうち 165 で、開口を引いた結果が閉じた形になった。`failed` の 3 要素は、開口の形が閉じていないので、manifold-rust が `NotManifold` で断った。
- B05 の柱の 59 要素は、`IfcIShapeProfileDef`・`IfcRectangleHollowProfileDef` の断面を使い、試作は写していない。
- 読み終えたときのメモリの頂点は、ファイルの 9〜13 倍で、建物モデルが足すのは 4〜30 % だった。多くは L1 が占める。
- すべての要素の形を `Shape` に写すと、36 MB 以上の 5 件で、写す時間の 3〜6 割の時間と、40〜116 MB が足された。形の評価（manifold-rust）は、1 件で 0.5〜0.8 秒だった。
