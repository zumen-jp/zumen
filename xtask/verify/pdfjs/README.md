# PDF アンダーレイのテストに使う pdf.js

`@zumen-jp/view2d` の DXF・DWG の `PDFUNDERLAY` 描画を Chromium で確かめるテスト（`bindings/js/test/browser/view2d.mjs`）のために、バージョンを固定した pdf.js 6.5.438（Apache-2.0）を入れます。テストのツールとしてだけ使い、zumen の依存や配布物には入れません。

```bash
npm ci --ignore-scripts --no-audit --no-fund --prefix xtask/verify/pdfjs
```
