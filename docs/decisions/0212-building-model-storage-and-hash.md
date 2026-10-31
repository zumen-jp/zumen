# 0212: 建物モデルは成分を要素の番号の列で持ち、ハッシュには持つ成分だけを種類の名前とともに入れ、層の向きを xtask で確かめる

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

[決定 0207](0207-building-model-components.md) は、建物モデルを要素と 19 の成分のグラフにし、ID とハッシュを [決定 0156](0156-drawing-stable-ids-and-preconditions.md) と同じ規則で作ると決めた。`zumen-building` を作るにあたり（#408）、成分の持ち方、要素の間の参照のハッシュへの入れ方、成分の種類を足したときのハッシュ、`zumen-building` が IFC の crate に依存しないことの確かめ方が決まっていなかった。

## 選択肢

ハッシュへの入れ方の選択肢。

1. 19 の成分を、持たないものも `なし` の印として、決まった順に入れる。
2. 要素が持つ成分だけを、種類の名前を前に付けて決まった順に入れ、終わりの印を置く。

選択肢 1 は、成分の種類を足すと、それを持たない要素のハッシュまで変わる。成分の種類は足すだけにする（決定 0207）ので、足すたびに版を上げることになる。

依存の確かめ方の選択肢。

1. `deny.toml` の `[bans].deny` に、`zumen-step` と `zumen-ifc` を直に使ってよい crate（`wrappers`）を書く。
2. `cargo xtask codegen --check` で、`cargo metadata` の依存をたどり、`zumen-building` から届かないことを確かめる。

選択肢 1 は、直の依存しか見ないので、`zumen-ifc` を使う crate を `zumen-building` が使っても見逃す。まだない crate を `wrappers` に書くと、cargo-deny が警告を出し続ける。

## 決定

ハッシュは選択肢 2、依存は選択肢 2 を採る。

- **持ち方。** 要素は `ElementRef`（書いた順の番号）で引き、ID と出所（インスタンスの番号と元の GlobalId）だけを持つ。成分は種類ごとに、要素の番号の順に並べた列（`Column`）で持ち、二分探索で引く。成分の中のほかの要素への参照も `ElementRef` で持つ。
- **ID。** `Building::push` が、GlobalId を読めて重ならなければ GlobalId を、ないか使えなければ `i` とインスタンスの番号を ID にする。使えない GlobalId は `building.global-id.invalid`、2 度目の GlobalId は `building.global-id.exists` を返す。インスタンスの番号の ID も重なれば、要素を足さずに `building.id.exists` を返す。
- **ハッシュ。** 版を 1 にし、`v1:` と 32 桁の 16 進で書く。要素が持つ成分だけを、決定 0207 の表の順に、種類の名前と値で入れ、空の名前で終える。要素自身の ID と出所は入れない。ほかの要素への参照は、指す先の ID で入れる。指す先の GlobalId が変われば、参照する要素のハッシュも変わる。形の項目の木は、積みを使わずに行きがけの順で入れる。文書のハッシュは決定 0156 と同じく元のバイト列から作り、文書の値（単位など）のハッシュは別に作る。
- **配置。** `world_placement` が、局所の変換を外側から掛けてプロジェクトの空間への変換を返す。たどる 1 段ごとに `depth` を 1 つ使い、終えたら戻す。循環は `building.placement.invalid`、配置を持たない親は `building.placement.missing`、解けない配置は `building.placement.unsupported` を返し、配置なしにする。
- **関係の索引。** `relations` が、所属（集約・入れ子・包含）の子、空間への参照、型の要素、開口、充填、グループの所属を、逆から引く表にする。成分の 1 つと参照の 1 つごとに `work` を 1 つ数える。
- **層の向き。** `xtask` に、依存してはならない crate の表を置く。`zumen-building` は `zumen-ifc` と `zumen-step` に、依存をたどっても依存しない。`zumen-building` の試験だけの依存もたどる。

## 影響

- IFC からの lift は、`zumen-ifc` の側から `zumen-building` を使って書く（#409）。
- 形の項目の型、`level`・`zone`・`classification` の作り方は、試料を得てから足す（決定 0207 の「試料を得てから決めること」）。成分を足しても、それを持たない要素のハッシュは変わらない。
- 依存してはならない crate の表は、層を足すときに足す。

## 参照

- [決定 0008](0008-model-layers-and-codecs.md)、[決定 0156](0156-drawing-stable-ids-and-preconditions.md)、[決定 0207](0207-building-model-components.md)
