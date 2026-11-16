# 3D のカーネルの候補の比較

[#414](https://github.com/zumen-jp/zumen/issues/414) の試作です。`zumen-geom3d` の合成の形の組（`zumen_geom3d::cases`）を、カーネルの候補ごとに評価し、体積・囲む箱・閉じているか・平面での切り口・出力の指紋（座標のビット）を比べます。採用条件は下の「サンプルで要った手当て」にまとめています。

[#5](https://github.com/zumen-jp/zumen/issues/5) では、公開サンプルの IFC（`samples/ifc`）で、速さと失敗の割合を測りました（[公開サンプルの IFC で測る](#公開サンプルの-ifc-で測る)）。結果の判断は [IFC と建物モデル](../../docs/formats/ifc.md) にあります。

この比較用の crate はコアのワークスペースに入れていません。選定した manifold-rust 0.19.1 は #420 でコアへ導入しました。[コアでの再測定](#コアでの再測定)と[現在の形状評価の契約](../../docs/architecture.md#建物の-3d-の形状評価)を参照してください。

## 3D カーネルの選定条件

ネイティブと wasm で同じ結果を返し、ビルドに C・C++ のツールを必要としないものを選びます。依存のライセンスには [コアの条件](../../AGENTS.md#範囲と権利) を適用します。

## 候補

| feature | 候補 | ブーリアン |
| --- | --- | --- |
| `own` | 自前と [i_overlay](https://crates.io/crates/i_overlay)（MIT OR Apache-2.0） | 断面に垂直に押し出した形から、同じ向きで厚さを覆う押し出しを引くときだけ、断面の 2D のブーリアンにする |
| `manifold` | [manifold-rust](https://crates.io/crates/manifold-rust) 0.19.1（Apache-2.0）。C++ の Manifold の Rust への移植 | メッシュのブーリアン |
| `csgrs` | [csgrs](https://crates.io/crates/csgrs) 0.20.1（MIT）。BSP の木 | メッシュのブーリアン |

- ブーリアンのない形（押し出し・回転・円の断面の掃引・面の集合・三角形の集合・写像・組）は、どの候補でも `src/primitives.rs` の同じコードで三角形にし、ブーリアンだけを候補に任せます。半空間は、manifold-rust は `trim_by_plane`、csgrs は大きな箱とのブーリアンにします。
- csgrs が依存する `core2` は、crates.io のすべてのバージョンが取り下げられていて、新しく解決できません。`Cargo.toml` で上流のリポジトリの 0.4.0 を当てています。
- C++ の Manifold を呼ぶ [manifold-csg](https://crates.io/crates/manifold-csg) は、作るときに C++ の Manifold を git で取得し、cmake と C++ のコンパイラーで作ります。wasm32-unknown-unknown では、libc++ のヘッダーを持つ LLVM の clang と wasm-cxx-shim の取得が要ります（`manifold-csg-sys` 3.5.105 の `build.rs`）。[ビルドの条件](#3d-カーネルの選定条件)に合わないので、比べていません。

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
- ブーリアンの前に、どの候補でも、点を `Tolerance::length`（同じ点とみなす距離）の格子にそろえます（`src/kernels.rs`）。公開サンプルの IFC で要った手当てです（下の「公開サンプルの IFC で測る」）。次の表は、この手当ての前に macOS で測りました。手当ての後（2026-11-03）は、Windows x64・Linux x86_64（WSL 2 の Ubuntu）・wasm32-wasip1 で流し、78 行の指紋がどれも同じで、組ごとの状態は表と同じでした。macOS では流し直していません。

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
- 指紋は、macOS arm64 と x86_64（Rosetta 2）のネイティブと、wasm32-wasip1（Node.js の WASI）で、78 行すべてが一致した。Linux と Windows では、手当ての後に流した（上）。
- コードの中の数（テストを除く）: manifold-rust は約 4.6 万行で、パニックしうる所（`unwrap`・`expect`・`panic!`・`assert!` など）が 646、std の三角関数などが 42、std の `HashMap` を使うファイルが 6（反復の順が実行ごとに変わりうる。3 回流した指紋は同じだった）。csgrs は約 1.4 万行で、それぞれ 95・121。i_overlay は約 1.7 万行で、20・3、`unsafe` が 49。

## 公開サンプルの IFC で測る

[#5](https://github.com/zumen-jp/zumen/issues/5) で、`samples/ifc` の公開サンプル 12 件（[#412](https://github.com/zumen-jp/zumen/issues/412)）を、候補ごとに測りました。国内のソフトの書き出し 9 件（[#513](https://github.com/zumen-jp/zumen/issues/513) の B01〜B09）は、公開の許諾がないので Git に入れず、手元で同じ手順で測った件数（大きさ・時間・メモリを含む）と種類（実体・属性の有無・書き方・値が 0 か）と、#513 に書いた書き出したソフトの名前とバージョンだけを書きます。ファイルは配りません。

- `samples [候補 | lift] <IFC>...` は、IFC を `zumen-ifc` で読み、建物モデルに写し（lift）、要素ごとの形状表現 `Body` を `zumen_geom3d::Shape` に写して（`src/ifc.rs`）、候補で評価する。開口（`IfcRelVoidsElement`）は、開口の `Body` を、なければ `Reference` を、母体の配置に対する相対の配置で写し、母体から引く。母体が部品の組（層ごとの壁など）なら、部品ごとに引く。
- 許容差は、弦の誤差を 1 mm、同じ点とみなす距離（格子の間隔）を、要素の `Body` の文脈が書いた精度（`IfcGeometricRepresentationContext` の `Precision`）にする。精度を書いていなければ 1e-9 m にする（[コアの設計](../../docs/architecture.md)）。どれもファイルの長さの単位に換算する。
- 行には、要素ごとの状態・体積・三角形の数・時間・指紋・切断の結果と時間・格子の間隔（メートル）を書く。状態は、閉じたメッシュなら `ok`、部品ごとに閉じた組なら `parts`、閉じた部分どうしが辺で接する形なら `touching`、閉じていても体積のない形（両面の面）なら `flat`、閉じていなければ `open`、評価しない形なら `unsupported:種類`、正しくない形なら `invalid:理由`、カーネルが失敗すれば `failed:理由`、パニックなら `panic` になる。
- 切断は、閉じた結果（`ok`・`parts`・`touching`）を、箱の高さの中ほどの水平の平面で切る（`zumen_geom3d::MeshSection`。どの候補のメッシュにも同じ切断を使う）。結果は `輪の数/点が 3 つ未満の輪の数` か、`empty`（切り口がない）・`panic`・失敗の名前。
- `#` で始まる行は、サンプルと候補ごとの要約で、読む・写す・形に写す時間と、メモリの最大（割り当てを数える）を書く。`lift` は評価せず、読んで写すまでを測る。
- `samples.sh` は、サンプルと候補ごとに別のプロセスで流し、15 分で打ち切る。サンプルのフォルダーは環境変数 `SAMPLES`（既定は `../../samples/ifc`）、候補は `CANDIDATES` で選ぶ。手元のサンプルは、`SAMPLES="$ZUMEN_SAMPLES/ifc"` のように渡す。`wasi-samples.mjs` は、wasm32-wasip1 に作った `samples` を Node.js の WASI で流し、`SAMPLES` のフォルダーを `/samples` に見せる。
- 環境で結果が変わらないかは、`file` の列をサンプルの名前にそろえ、時間の列（`micros_body`・`micros_full`・`micros_section`）を除いた行で比べる。
- `ifcopenshell_volumes.py` は、`ok`・`parts`・`touching` の要素の体積を、IfcOpenShell（テストのツールで、依存にしない）が開口を引いて作った形の体積と照らす。IfcOpenShell 0.9.0.post1 は仮想環境に入れ、その Python で流す（`-I` は利用者の site-packages を見ないが、仮想環境のものは見る）。

```bash
cargo build -q --release --bin samples
./samples.sh <出力のフォルダー> ./target/release/samples
SAMPLES="$ZUMEN_SAMPLES/ifc" CANDIDATES=manifold-rust ./samples.sh <出力のフォルダー> ./target/release/samples
cargo build -q --release --bin samples --target wasm32-wasip1
./samples.sh <出力のフォルダー> node --no-warnings wasi-samples.mjs target/wasm32-wasip1/release/samples.wasm
uv venv --python 3.13 --no-python-downloads <仮想環境>
uv pip install --python <仮想環境> ifcopenshell==0.9.0.post1
<仮想環境の python> -I ifcopenshell_volumes.py <出力のフォルダー> manifold-rust
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

### サンプルで要った手当て

6 のほかは、どの候補にも同じに当てます（6 は manifold-rust だけ）。

1. 断面と面を三角形にする手順（earcut）は、同じ直線の上の点を落とし、穴の頂点を通る辺を作るので、T 字の継ぎ目が残る。逆向きの辺がない辺だけを、その上の点で分ける（`src/primitives.rs`）。分けないと、押し出しのメッシュが閉じず、manifold-rust が `NotManifold` で断った。
2. 面積のない面（点が 1 直線に並ぶ面など）は、要素を断らずに飛ばす。国内の書き出しの 214 要素（多くは設備の継手）にあった。飛ばすと、132 要素が閉じた形（`ok` か `parts`）になった。
3. GNI の Archicad 28 のサンプル（公開元が保存し直したもの）は、開口の形を `Body` でなく `Reference` に書く。`Body` がなければ `Reference` を使う。
4. 層ごとの壁など、閉じた部品の組の母体は、部品ごとに開口を引く。組を 1 つのメッシュにして引くと、閉じた形にならない。
5. ブーリアンの前に、点を `Tolerance::length` の格子にそろえる（`src/kernels.rs`）。BIM Whale の TallBuilding の壁（インスタンス `#2401`）は、面が開口の面から 1.6e-13 mm ずれていて、3 つの開口を順に引くと、manifold-rust が 1 つの開口のうち 915 × 200 × 305 mm を引かなかった。逆の順に引く・先に開口の和を取る・格子にそろえる、のどれでも正しい体積になる。1e-9 m の格子にそろえると、csgrs のパニックも 104 から 5 に減った。格子の間隔は、文書の精度（なければ 1e-9 m）にする（[コアの設計](../../docs/architecture.md)）。1e-9 m の格子と比べると、manifold-rust の状態はすべて同じで、体積が変わったのは 47 要素（GNI の model_125 の 46・林野庁の 1）、相対の差は 7.6e-6 以下だった。格子は、要素の局所の座標で作る。
6. manifold-rust のブーリアンは、`_with_engine` で `Exact` のエンジンを明示する。既定のエンジンはプロセス全体の設定で、ほかから変えられる。半空間で切る `trim_by_plane` はエンジンを渡せないので、既定が `Exact` であることを確かめてから呼ぶ。`trim_by_plane` は std の `powi` も通るので、#420 では使わず、`Booleans::clip` の既定（csgrs が使う）と同じく大きな箱とのブーリアンにする。

### 結果（2026-11-03）

Windows 11 x64（AMD Ryzen 5 3600XT、メモリ 32 GB）、Rust 1.99.1、Node.js 24.21.0 で測りました。Linux は、同じ機械の WSL 2 の Ubuntu（x86_64）で、同じ Rust のバージョンで流し、行を Windows と比べました。時間はネイティブで、サンプルと候補ごとに別のプロセスで流した値です。

格子の間隔（文書の精度）は、林野庁・BIM Whale・GNI が 1e-5 m、中野区が 2.54e-8 m、Text2BIM が 1e-8 m でした。公開サンプルの結果は、次のとおりです（自前の列は、評価しない種類と件数）。

| サンプル | 要素 | 読む・写す（ms） | 最大（MB） | 形（manifold-rust） | 開口のある要素 | 開口を引いた結果: manifold-rust / csgrs / 自前 | 形・開口・切断の時間（ms） |
| --- | ---: | ---: | ---: | --- | ---: | --- | ---: |
| `bim-whale/LargeBuilding` | 122 | 24・38 | 17.5 | ok 67・parts 44・open 11 | 15 | ok 15 / open 15 / boolean 10・half_space 5 | 37・27・5 |
| `bim-whale/TallBuilding` | 58 | 11・19 | 9.0 | ok 36・parts 22 | 12 | ok 12 / open 12 / half_space 12 | 6・10・2 |
| `gni/model_125` | 416 | 58・60 | 39.5 | ok 177・parts 105・open 134 | 57 | ok 24・parts 33 / open 53・panic 4 / boolean 57 | 113・223・8 |
| `gni/model_18` | 194 | 33・45 | 20.0 | ok 104・parts 66・touching 17・open 7 | 17 | ok 17 / open 17 / boolean 15・half_space 2 | 61・40・9 |
| `gni/model_9` | 148 | 10・12 | 6.8 | ok 96・parts 44・touching 8 | 12 | ok 12 / open 12 / boolean 12 | 25・51・4 |
| `nakano/nakano_20F_Restaurant` | 62 | 10・25 | 4.4 | ok 61・open 1 | 0 | - | 12・0・3 |
| `nakano/nakano_6F_chapel` | 208 | 7・9 | 4.2 | ok 3・open 9・flat 196 | 0 | - | 2・0・0 |
| `nakano/nakano_ConcertHall` | 80 | 3・3 | 1.7 | ok 14・flat 66 | 0 | - | 1・0・0 |
| `nakano/nakano_Outside` | 37 | 2・2 | 1.2 | ok 24・touching 5・flat 8 | 0 | - | 1・0・0 |
| `nakano/nakano_entrance` | 188 | 11・15 | 7.0 | ok 143・touching 24・open 4・flat 17 | 0 | - | 6・0・1 |
| `rinya/wood-office-architecture` | 342 | 35・53 | 21.8 | ok 304・parts 18・touching 20 | 58 | ok 58 / ok 4・open 51・panic 3 / boolean 58 | 38・130・11 |
| `text2bim/sample_model` | 19 | 7・9 | 3.7 | ok 9・touching 3・open 7 | 0 | - | 1・0・0 |

- manifold-rust は、開口のある 171 要素のすべてで、開口を引いた結果が閉じた形（`ok` か `parts`）になった。パニックと失敗はなかった。
- csgrs は、開口を引いた結果が 4 要素しか閉じず（BSP の T 字の継ぎ目）、model_125 の 4 要素と林野庁の 3 要素でパニックした。wasm32 では、パニックでサンプルの全体が止まった。
- 自前は、開口のある要素のどれも評価しない（壁の開口は押し出しの向きが母体と違い、半空間での切断もある）。
- `open` と `flat` は、面の集合で書いた家具・設備・中野区の地形などに出た。試作では、これらの要素に開口がなく、ブーリアンを通らなかった。#420 のコア実装では、閉じていない形と、体積のない閉じた形のどちらも表示用に保ち、ブーリアンには渡さない。
- 切断は、manifold-rust の閉じた 1,414 要素のすべてで、失敗とパニックがなかった。1,390 要素は輪になった。6 要素は切り口がなかった（開口で上下に分かれた壁と、段の部品を並べた階段で、高さの中ほどに材料がない）。18 要素は、点が 3 つ未満の輪を含んだ（中野区の、点群から作った面の集合。ブーリアンを通らない形で、どの候補でも同じ）。時間は、1 件で 11 ms 以下、1 要素で 1.6 ms 以下だった。
- ネイティブで 2 度流した 5,622 行と、Linux の 5,622 行は、すべて同じだった。wasm32-wasip1 の 4,864 行（manifold-rust と自前の 1,874 行ずつを含む）は、ネイティブと座標のビットまで同じだった。残りの 758 行は、wasm32 でパニックして止まった csgrs の 2 組の分で、比べられなかった。
- wasm32 の時間は、読む・写すがネイティブの約 1.8 倍、形と開口が約 1.4 倍だった（開口を引く時間は、1 件でネイティブ 0.22 秒以下・wasm32 0.28 秒以下）。メモリの最大は、ネイティブの約 0.7 倍だった（ポインターが 32 ビット）。

IfcOpenShell 0.9.0.post1 の既定の設定（開口を引く）と、manifold-rust の `ok`・`parts`・`touching` の 1,414 要素の体積を照らしました。

| 相対の差 | 要素 |
| --- | ---: |
| 1e-6 以下 | 1,255 |
| 1e-6 を超え 1e-4 以下 | 99 |
| 1e-4 を超え 1e-3 以下 | 1 |
| 1e-2 を超える | 3 |
| 体積が 1e-5 m³ 未満（比べない） | 56 |

- 1e-2 を超える 3 要素は、どれも GNI の model_125 の床で、部品の 1 つの面が、幅 100 mm の切り込みを、辺を重ねて書いた多角形だった。その部品の体積が違う（`#26857` の部品で、試作 7.83 m³・IfcOpenShell 6.53 m³）。面の座標から求めた体積は、3 要素のどの部品でも試作の値と合った。IfcOpenShell がこの面をどう三角形にしたかは、確かめていない。
- 1e-6 を超え 1e-3 以下の 100 要素のうち 76 は、中野区の entrance の要素だった。形は平面直角座標（原点から約 3.6 万 m）のまま書かれていて、面は平ら（ずれ 1e-11 m 以下）なので、体積の式の桁落ちと推定する。ほかの 24 は model_125 の要素で、格子（0.01 mm）にそろえて点が動いた分だった（1e-9 m の格子では 1 要素だけだった）。

国内のソフトの書き出し（Git に入れていない）の、manifold-rust での結果は、次のとおりです。格子の間隔は、各ファイルが書いた精度で、FILDER CeeD は精度を書いていないので 1e-9 m です。

| ID | 書き出したソフト | 大きさ（MB） | 読む・写す（ms） | 読む・写すの最大（MB） | 形 | 開口のある要素 | 開口を引いた結果 | 切断（ms） |
| --- | --- | ---: | ---: | ---: | --- | ---: | --- | ---: |
| B01 | CADEWA Real2011 | 1.1 | 22・30 | 14.1・14.6 | ok 132・touching 1・open 37 | 0 | - | 1 |
| B02 | ARCHICAD 20 日本語版 | 2.1 | 41・66 | 23.9・28.0 | ok 758・touching 3・open 9 | 30 | ok 30 | 7 |
| B03 | ARCHICAD 20 日本語版 | 0.7 | 14・17 | 7.5・8.4 | ok 150・open 3 | 8 | ok 8 | 1 |
| B04 | ArchiCAD 15 日本語版 | 6.5 | 136・179 | 83.1・86.8 | ok 236・parts 14・touching 8・open 119 | 28 | ok 16・touching 12 | 14 |
| B05 | GLOOBE Architect 2024 | 36.2 | 855・1,228 | 457.3・474.8 | ok 1,576・parts 180・touching 197・open 11・flat 4 | 102 | ok 99・failed 3 | 87 |
| B06 | Rebro2025 | 48.7 | 1,013・1,962 | 506.8・635.5 | ok 2,015・parts 652・touching 12・open 142 | 0 | - | 93 |
| B07 | CADWe'll Linx V6 | 57.2 | 1,378・2,359 | 738.7・866.9 | ok 2,421・parts 438・open 303 | 0 | - | 79 |
| B08 | CADEWA Smart V5.0 | 63.2 | 1,189・2,122 | 602.5・787.0 | ok 1,950・parts 217・touching 35・open 513 | 0 | - | 65 |
| B09 | FILDER CeeD | 245.3 | 5,179・7,337 | 2,867.2・2,995.7 | ok 2,096・parts 97・touching 52・open 488・flat 15 | 0 | - | 59 |

- 9 件とも、パニックせず、途中で止まったサンプルもなかった。形を写せた開口のある 168 要素のうち 165 で、開口を引いた結果が閉じた形になった。`failed` の 3 要素は、開口の形が閉じていないので、manifold-rust が `NotManifold` で断った。
- B05 の柱の 59 要素は、`IfcIShapeProfileDef`・`IfcRectangleHollowProfileDef` の断面を使い、試作は写していない（開口のある要素に数えていない）。
- 1e-9 m の格子で測った結果と比べて、状態はすべて同じだった。Linux の 14,943 行も、Windows と同じだった。
- 切断は、閉じた 13,237 要素のすべてで、失敗とパニックがなかった（6 要素は切り口がない）。時間は、1 件で 0.1 秒以下、1 要素で 26 ms 以下だった。
- 読み終えたときのメモリの最大は、ファイルの 9〜13 倍で、建物モデルが足すのは 4〜31 % だった。多くは L1 が占める。
- すべての要素の形を `Shape` に写すと、36 MB 以上の 5 件で、写す時間の 3〜6 割の時間と、40〜116 MB が足された。形の評価（manifold-rust）は、1 件で 0.5〜0.8 秒だった。

## コアでの再測定

#420 の `zumen-building::evaluation` と `zumen-geom3d::ManifoldKernel` を、2026-11-16 に Windows 11 x64、Ryzen 5 3600XT、Rust 1.99.1 の release build で測りました。並行する開発環境で 3 回測った中央値で、時間は目安です。上の試作とは読み取り・形の保持・予算の検査が違うため、速度の比較には使いません。

公開 12 件の形を持つ 2,521 要素のうち 2,502 要素（99.25%）を描け、欠落や集合演算の代替がない要素は 2,493（98.89%）でした。開口自身もこの数に含めます。`geometry` は形の根がある要素、`drawn` は 1 つ以上の部品がある要素、`complete` は描けたうえで欠落や代替がない要素です。Body のない形の根も分母に入るので、表示対象だけの対応率とは区別します。

| 公開サンプル | geometry / drawn / complete | 読む・lift・評価（ms） | 読む・lift の追加確保の最大（MB） | 評価の追加確保の最大（MB） |
| --- | ---: | ---: | ---: | ---: |
| bim-whale/LargeBuilding | 182 / 182 / 182 | 23.1・983.7・323.4 | 23.089 | 3.363 |
| bim-whale/TallBuilding | 84 / 84 / 84 | 13.1・188.5・21.3 | 9.920 | 0.453 |
| gni/model_125 | 629 / 629 / 629 | 64.5・14154.2・545.0 | 61.236 | 10.847 |
| gni/model_18 | 280 / 278 / 273 | 33.3・1729.0・169.6 | 26.597 | 3.953 |
| gni/model_9 | 205 / 200 / 200 | 11.1・382.8・108.9 | 10.394 | 2.064 |
| nakano/20F_Restaurant | 62 / 62 / 62 | 8.0・35.6・16.3 | 4.642 | 0.381 |
| nakano/6F_chapel | 208 / 208 / 208 | 7.3・101.4・6.1 | 4.951 | 0.338 |
| nakano/ConcertHall | 80 / 80 / 80 | 3.0・23.2・1.4 | 1.975 | 0.126 |
| nakano/entrance | 188 / 188 / 188 | 12.2・142.8・11.5 | 7.715 | 0.516 |
| nakano/Outside | 37 / 37 / 37 | 2.2・8.2・1.4 | 1.286 | 0.089 |
| rinya/wood-office-architecture | 526 / 520 / 520 | 38.6・3295.0・258.6 | 31.393 | 4.141 |
| text2bim/sample_model | 40 / 34 / 30 | 7.2・31.1・4.0 | 3.951 | 0.166 |

確保量は `System` のアロケーターで測るヒープの追加量で、MB は 1,000,000 バイトです。読む・lift は入力のバイト列を確保した後から、評価は L1 を捨てた後から測ります。OS の常駐メモリや、予算の論理的な予約量ではありません。公開入力の計測は確保量の予算 64 GiB、作業量 100,000,000,000 を指定し、予算停止の試験は別に行います。L1 の省メモリ化は #541 で扱います。

母体自身が部品を持つ開口付きの 171 要素は、すべて閉じた結果になりました。Text2BIM の形を持たない親の開口は子部品へ適用し、閉じていない 4 要素では 6 回の差を断って元形を残しました。GNI model_18 では未対応の形の項目を 21 件診断しました。描いたメッシュは閉じた立体に限らず、開いた面や厚さのない両面も含みます。

IfcOpenShell 0.9.0.post1（Python 3.13.17）の既定の開口を適用する設定と、完全に評価できた閉じた要素を別に照合しました。開口自身の 598 要素と、体積が 1e-5 m³ 未満の 24 要素を除き、1,357 要素を比較しました。

| 相対差の上限 | 一致した要素（累積） |
| --- | ---: |
| 1e-6 | 1,332 |
| 1e-4 | 1,354（99.78%） |
| 1e-3 | 1,354 |
| 1e-2 | 1,354 |

残る 3 要素は、上の試作で記録した GNI model_125 の床（#25373・#26857・#26630）で、相対差は 33.89%・5.65%・4.79% です。今回は外部実装の三角形分割の理由を再調査しておらず、一致した扱いにはしていません。子部品への開口が不足していた Text2BIM の 2 要素は修正後に一致しました。外部実装のソースと IFC のスキーマは使わず、別に導入したバイナリの形の出力だけを照合します。

環境間の照合は、31 の合成形状と、開口付きの壁・床・建具・子部品の建物、診断、予算と中断を並べた 249,120 バイトです。Windows x64 と wasm32-wasip1 の SHA-256 は、ともに `104b31404e83d811221465dc106df10b2d861a77aa13ac9586d339f3ce8f7389` でした。同じ記録を通常の CI で Linux x64・Windows x86/x64・macOS arm64・wasm に照らします。公開サンプルの環境間の全メッシュの一致は、この合成入力の検査とは区別します。

依存の集合演算の経路では、`boolean3` のハッシュ表はキーの照会に使い、`disjoint_sets` のラベルは順序を持つ元の列へ戻します。`boolean3_kernels` の集合の走査は独立した頂点への整数の巻き数を求めるため、浮動小数の加算順を変えません。コア側の点・辺・キャッシュには順序を持つ表を使います。上流のコードは変更せず、Exact の指定、入力制限、局所の格子、T 字の継ぎ目、退化した面をコアのラッパーで扱います。

再測定は、リポジトリ外の `ZUMEN_SAMPLES`、または公開入力を指す `ZUMEN_SAMPLES` を設定し、相対パスを辞書順に渡します。入力の名前と属性値は出力しません。`--elements` は外部照合用のインスタンス番号と体積も出すので、受領図面ではログを公開しません。

```sh
cargo run --release -p zumen-ifc --example mesh_audit -- --elements <相対パス...> > mesh-results.txt
python crates/zumen-ifc/tests/check_mesh_volumes.py mesh-results.txt samples/ifc
cargo build -p zumen-building --example evaluation_contract
python crates/zumen-building/tests/check_evaluation.py target/debug/examples/evaluation_contract
```
