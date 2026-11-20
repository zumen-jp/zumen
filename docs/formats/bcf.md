# BCF

BCF（BIM Collaboration Format）の bcfzip を [レビューの文書](review.md) と相互に変換します。XSD は同梱しません。

## 読み書き

- `bcf_read` は `bcf.version` の `VersionId` で 2.1 と 3.0 を判別する。topic を指摘、comment をコメント、viewpoint を 3D の場所に写す。Guid は小文字にそろえる。対応表にない状態は `open` とし、元の文字列を `status_label` に残す。
- `bcf_write` の版は、引数、元の bcfzip の版、3.0 の順に選ぶ。元と違う版への書き戻しは `bcf.conversion.unsupported` で断る。別の版を新しく作るときは元の bcfzip を渡さない。
- 同じ Guid の変えていない topic は元のフォルダーをそのまま書く。変えた topic でも未知の要素・属性、文書の参照、変えていないコメントと viewpoint を残す。カメラを変えた viewpoint は新しいファイルにし、合わなくなったスナップショットを引き継がない。
- [XML の規則](xml.md)で UTF-16 の部品も読む。元の markup が UTF-8 でない topic を変更すると、未知の要素と属性を保って書き戻せないため、`bcf.encoding.unsupported` で止める。変更しない topic は、Shift_JIS・UTF-16 とも原本のまま書く。
- 書けない場所・図形・要素・照合の由来は診断にする。作者や日時などの必須値は呼び出し側から受け取り、補った値を報告する。値が足りなければ失敗する。
- 2.1 はコメントの本文が空でも書き、カメラの縦横比を書かない。元がないとき、2.1 は拡張を省き、3.0 は書き出した値から `extensions.xml` を作る。往復と欠落の診断は [適合テスト](../../bindings/conformance/cases/bcf.json) で確かめる。

## 確かめた度合い

自作の UTF-16LE・UTF-16BE と Shift_JIS の markup で、未変更の topic のバイト保持と、題を変えたときの保存拒否を検査します。UTF-8 の未知の要素・属性の保持も既存の回帰テストで確かめます。文字コードを変えた BCF の外部照合はしていません。

| 形式 | 向き | 段 |
| --- | --- | --- |
| `bcf` | 読み | `synthetic` |
| `bcf` | 書き | `external` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-bcf)にあります。自作 BCF 2.1 の編集出力と 2.1・3.0 の新規出力を Smino.Bcf.Toolkit 3.2.0 で読み、指摘・コメント・視点の GUID と参照・値、透視と平行のカメラ、選択要素、可視性、画像を固定期待値と照らしました。3 ファイル・12 指摘の範囲と、比較器の反例は[検証キット](../../xtask/verify/bcf/README.md)に記録しています（#522）。

- 外部実装が再保存した 3 ファイルも読み、同じ期待値と照らした。元が自作の指摘なので、読みの段は `synthetic` を維持する。未知の XML・文書参照・切断面・色・文字コードの変種の外部照合と、製品の表示は未確認。
- 2026-11-16 に buildingSMART/BCF-XML の `release_3_0` を調べた。根の LICENSE は CC BY-ND 4.0 で、個々の例の別の許諾は未確認のため、公開 BCF の例は収録していない。参照先とライセンスの指紋は検証キットの説明に記録した。
- Autodesk Build の取り込み・書き出しには、[オフラインの検証キット](../../xtask/verify/acc/README.md)を使う（#686・#787）。公開 IFC の要素に合う BCF 2.1 を作り、題・状態・コメントを書き戻して、選択要素・カメラ・画像の保持を zumen で読み直す。Autodesk Build は合成した 4 件すべてを取り込んだ。題・説明は一致し、コメントには作者の表記が加わった。状態は Open・Resolved が未完了、Closed が完了になった。4 件とも元の画像が添付とサムネイルになった。検証 01 と 04 は壁のピンを確認した。製品から書き出した BCF の再読は #686 で追う。[4 件の取り込み結果](https://github.com/zumen-jp/zumen/issues/686#issuecomment-6943687042)。
