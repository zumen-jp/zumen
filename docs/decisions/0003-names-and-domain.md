# 0003: パッケージの名前を org から切り離し、識別子にドメイン zumen.dev を使う

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- 旧版の org の名前（`zumenjs`）は、npm のスコープ（`@zumenjs/*`）と URL に焼き付いていた。そのため org の名前を変えると、すべての識別子が変わった。
- 2026-10-24 に、各レジストリの名前の空きを確かめた。
  - 空いていた: crates.io の `zumen`・`zumen-*`、NuGet の `Zumen`、pub.dev・RubyGems の `zumen`、Packagist の `zumen-jp/zumen`
  - 旧版が公開済み: PyPI の `zumen`
  - 他人が使用中: npm の素の `zumen`（ほかの人の CLI）
- ドメイン `zumen.dev` を取得した（Cloudflare）。npm の org `zumen-jp` も作った。

## 選択肢

1. **すべての識別子に GitHub の org の名前を使う**（`io.github.zumen-jp`、`github.com/zumen-jp/...` など）。手間はないが、org を変えるとまた識別子がすべて変わる。
2. **org を持たない名前とドメインを使う。** org に縛られるのは、npm のスコープと GitHub の URL だけになる。

## 決定

2 を選ぶ。

| 場所 | 名前 |
| --- | --- |
| crates.io | `zumen`・`zumen-*` |
| PyPI | `zumen`（旧版から続ける） |
| npm | `@zumen-jp/*` |
| NuGet | `Zumen`・`Zumen.*`（接頭辞の予約を申請する） |
| pub.dev・RubyGems | `zumen` |
| Packagist | `zumen-jp/zumen` |
| Maven | groupId `dev.zumen` |
| Go | `zumen.dev/go`（`go-import` の meta で鏡のリポジトリを指す） |
| Apple | バンドル ID `dev.zumen.*` |
| 文書 | `https://zumen.dev` |
| CLI | コマンド `zumen`。Homebrew は `zumen-jp/tap/zumen` |

- crates.io の名前は、中身のある最初の版で取る。名前だけの予約は、crates.io の規約で禁じられている。
- 旧版の `@zumenjs/*`（0.1.1）は、新しい名前で公開したあとに非推奨にし、移った先を案内する。
- 利用者に `npx zumen` を案内しない。ほかの人のパッケージが動いてしまうため。

## 影響

- 公開の前に、次を用意する。
  - `zumen.dev` の DNS
  - `go-import` の meta
  - 文書のサイトの置き場所
- NuGet の接頭辞の予約を申請する。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1)
- [0001](0001-rebuild-in-zumen-jp.md)
