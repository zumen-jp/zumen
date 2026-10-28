// サイトの中で使い回す文と表。お知らせ・対応形式は、ここを直す。

export const repo = 'https://github.com/zumen-jp/zumen';
export const legacy = 'https://github.com/zumen-jp/zumen-legacy';
export const decision = (file: string) => `${repo}/blob/main/docs/decisions/${file}`;

export const nav = [
  { href: '/features/', label: '特長' },
  { href: '/formats/', label: '対応形式' },
  { href: '/platforms/', label: '対応環境' },
  { href: '/news/', label: 'お知らせ' },
  { href: '/faq/', label: 'よくある質問' },
];

export type Tag = '開発' | '決定' | 'お知らせ';

export interface News {
  date: string;
  tag: Tag;
  // 本文は HTML。リンクを含めてよい。
  html: string;
}

export const news: News[] = [
  {
    date: '2026-10-28',
    tag: '開発',
    html: `図面モデルの変更を DXF・DWG に書き戻す apply を加えました（<a href="${repo}/pull/223">#223</a>）。`,
  },
  {
    date: '2026-10-28',
    tag: '開発',
    html: `SXF の SFC と Jw_cad の JWW を、図面モデルへ移せるようになりました（<a href="${repo}/pull/181">#181</a>・<a href="${repo}/pull/201">#201</a>）。`,
  },
  {
    date: '2026-10-27',
    tag: '決定',
    html: `DWG への書き戻しと、R2004 の入れ物の書き方を決めました（<a href="${repo}/blob/main/docs/decisions/README.md">決定 0052〜0070</a>）。`,
  },
  {
    date: '2026-10-26',
    tag: '開発',
    html: `DXF と DWG（R2000）を読む最初の経路を置きました（<a href="${repo}/pull/31">#31</a>・<a href="${repo}/pull/38">#38</a>）。`,
  },
  {
    date: '2026-10-24',
    tag: 'お知らせ',
    html: `新しい org の zumen-jp で、zumen の作り直しを始めました（<a href="${decision('0001-rebuild-in-zumen-jp.md')}">決定 0001</a>）。`,
  },
];

// ○ 使える・△ 開発中（未公開）・― 予定・× 扱わない
export type Mark = '○' | '△' | '―' | '×';

export interface Format {
  name: string;
  about: string;
  read: Mark;
  write: Mark;
  place: string;
  // トップの早見表に出す
  top?: boolean;
}

export const formats: Format[] = [
  { name: 'SXF', about: '電子納品の CAD データ（SFC・P21）', read: '△', write: '△', place: '核', top: true },
  { name: 'JWW', about: 'Jw_cad for Windows の図面', read: '△', write: '―', place: '核', top: true },
  { name: 'JWC・JWS', about: 'Jw_cad for DOS の図面、Jw_cad の図形', read: '―', write: '―', place: '核' },
  { name: 'DXF', about: 'AutoCAD の交換形式', read: '△', write: '△', place: '核', top: true },
  { name: 'DWG', about: 'AutoCAD の図面', read: '△', write: '△', place: '核', top: true },
  { name: 'PDF', about: '図面の PDF', read: '―', write: '―', place: '核', top: true },
  { name: 'IFC', about: 'BIM・土木のモデル', read: '―', write: '―', place: '核', top: true },
  { name: 'ST-Bridge', about: '構造のモデル', read: '―', write: '―', place: '外部のプラグイン' },
  {
    name: 'BE-Bridge・CEDXM・Stem',
    about: '設備の CAD データ・機器のデータ',
    read: '×',
    write: '×',
    place: '権利を確かめられるまで扱わない',
  },
];

export const hostApps = [
  'Revit',
  'AutoCAD',
  'Civil 3D',
  'Dynamo',
  'pyRevit',
  'Rhino・Grasshopper',
  'Excel',
  'QGIS',
  'ArcGIS Pro',
  'Blender',
  'FreeCAD',
  'SketchUp',
  'Unity',
];

export interface Faq {
  q: string;
  // 答えは HTML。リンクを含めてよい。
  a: string;
  top?: boolean;
}

export const faqs: Faq[] = [
  {
    q: '今すぐ使えますか。',
    a: `作り直しの開発中で、パッケージはまだ公開していません。SXF・JWW・DXF・DWG・PDF を今すぐ読み書きしたい場合は、<a href="${legacy}">旧版の zumen-legacy</a> を使ってください。`,
    top: true,
  },
  {
    q: '費用はかかりますか。会社の業務で使えますか。',
    a: 'MIT License または Apache License 2.0 で公開するので、費用はかかりません。社内のシステムや製品に組み込んで使えます。',
    top: true,
  },
  {
    q: '図面を外部のサーバーに送りますか。',
    a: '送りません。図面は、利用者のパソコン・ブラウザー・社内のサーバーの中で読みます。',
    top: true,
  },
  {
    q: '電子納品の SXF や Jw_cad の図面を読めますか。',
    a: 'SXF（SFC・P21）と JWW を、日本の実務の形式として先に作っています。進み具合は<a href="/formats/">対応形式</a>にあります。',
  },
  {
    q: 'どの言語・ソフトから使えますか。',
    a: 'Rust・TypeScript・JavaScript・Python・.NET・C・C++ を中心に、Java・Swift・Go・VBA などにも出します。Revit・AutoCAD・Excel などの中からも使えるようにします。詳しくは<a href="/platforms/">対応環境</a>にあります。',
  },
  {
    q: '読めなかった図形はどうなりますか。',
    a: '黙って捨てず、読めなかったものの種類・件数・場所を診断として返します。',
  },
  {
    q: '不具合の報告や、開発への参加はできますか。',
    a: `できます。<a href="${repo}/issues">GitHub の issue</a> で受け付けます。参加の手順は <a href="${repo}/blob/main/CONTRIBUTING.md">CONTRIBUTING.md</a> にあります。脆弱性は <a href="${repo}/blob/main/SECURITY.md">SECURITY.md</a> の手順で知らせてください。`,
  },
];
