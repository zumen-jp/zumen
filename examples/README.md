# サンプル

| ファイル・フォルダー | 中身 |
| --- | --- |
| [render-svg.mjs](render-svg.mjs) | 図面・範囲・色と、必要ならフォントを指定して SVG を作る Node.js の例。手順は [docs/render.md](../docs/render.md) |
| [render-png.mjs](render-png.mjs) | 図面・範囲・余白・色と、必要ならフォントを指定して PNG を作る Node.js の例。手順は [docs/render.md](../docs/render.md) |
| [tier3](tier3) | Ruby・PHP・PowerShell で図面を開くサンプルと、AutoCAD（AutoLISP）・MATLAB で確認した利用例。[実行手順](../docs/tier3.md) |

毎晩の CI は Ruby・PHP・PowerShell で同じ公開図面を開き、形式と図形の数を[共通の期待値](tier3/expected.txt)と照らします。
