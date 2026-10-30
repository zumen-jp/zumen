# プロファイル

基準・要領・社内標準を、コードとは別の、版のあるデータとして持ちます。形は [決定 0138](../docs/decisions/0138-profiles-as-versioned-data.md) に、照合は [決定 0173](../docs/decisions/0173-profile-checks.md) にあります。

| id | 版 | 対象 |
| --- | --- | --- |
| `mlit-cad` | [2025-12](mlit-cad/2025-12/profile.toml) | CAD製図基準（土木、令和 7 年 12 月） |

## 直し方

- 元は `<id>/<edition>/profile.toml` です。直したら `cargo xtask codegen` で `profile.json` を作り直し、両方をコミットします。
- 値には出典の文書・節・項を書きます。出典が列挙しない値は `basis = "chosen"` にします。
- 基準の新しい版は、新しいフォルダーにします。同じ版のデータを直したら `revision` を上げます。
- 仕様の文を写さず、値と、節・表の番号だけを書きます。

## ライセンス

このフォルダーのデータは [CC BY 4.0](LICENSE.md) です。
