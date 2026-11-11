// サイトの中で使い回す文と表。お知らせ・対応形式は、ここを直す。

export const repo = 'https://github.com/zumen-jp/zumen';
export const releaseVersion = '0.2.0';
export const releaseUrl = `${repo}/releases/tag/v${releaseVersion}`;
export const releasedSource = (file: string) => `${repo}/blob/v${releaseVersion}/${file}`;

// ナビの並び。ビューアーのヘッダーとの一致を、組み立ての検査がこの { href, label } の形で読む。
export const nav = [
  { href: '/viewer/', label: '図面を試す' },
  { href: '/features/', label: '特長' },
  { href: '/formats/', label: '対応形式' },
  { href: '/platforms/', label: '対応環境' },
  { href: '/news/', label: 'お知らせ' },
  { href: '/faq/', label: 'よくある質問' },
] as const;

export interface PageInfo {
  // 見出しの上に添える英語の肩書き
  en: string;
  // 見出しの下と、ほかのページの終わりの案内に出す説明
  lead: string;
}

// ナビの各ページの肩書きと説明。下層のページの見出しと、ページの終わりの「ほかのページ」に使う。
export const pages: Record<(typeof nav)[number]['href'], PageInfo> = {
  '/viewer/': { en: 'Viewer', lead: '公開サンプルや手元の図面を、ブラウザーの中だけで開きます。' },
  '/features/': { en: 'Features', lead: 'zumen が何をするライブラリ群か、どのように作っているかを説明します。' },
  '/formats/': { en: 'Formats', lead: '形式ごとの、読む・書くの状況と、実装を置く所です。' },
  '/platforms/': { en: 'Platforms', lead: '言語ごとの約束、OS とブラウザーの下限、公開済みのパッケージです。' },
  '/news/': { en: 'News', lead: 'リリースと、開発の大きな節目を知らせます。' },
  '/faq/': { en: 'FAQ', lead: 'zumen について、よくいただく質問と答えです。' },
};

// お知らせは、リリースと大きな節目だけを書く。日々の変更は GitHub の PR とリリースノートに任せる。
export type Tag = 'リリース' | 'お知らせ';

export interface News {
  date: string;
  tag: Tag;
  // 本文は HTML。リンクを含めてよい。
  html: string;
}

export const news: News[] = [
  {
    date: '2026-11-10',
    tag: 'リリース',
    html: `v${releaseVersion} を公開しました。図面・BIM の読み書き、土木・地理・点群の読み取りを、Rust・JavaScript・Python・.NET・C・C++・CLI から使えます。<a href="/platforms/#packages">配布先と導入手順</a>、<a href="${releaseUrl}">リリースの内容</a>をご覧ください。`,
  },
  {
    date: '2026-10-24',
    tag: 'お知らせ',
    html: `新しい zumen の開発を、GitHub の <a href="${repo}">zumen-jp/zumen</a> で始めました。`,
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
  { name: 'SXF', about: '電子納品の CAD データ（SFC・P21・SFZ・P2Z）', read: '○', write: '○', place: 'コア', top: true },
  { name: 'JWW', about: 'Jw_cad for Windows の図面', read: '○', write: '○', place: 'コア', top: true },
  { name: 'JWC・JWS', about: 'Jw_cad for DOS の図面、Jw_cad の図形', read: '―', write: '―', place: 'コア' },
  { name: 'DXF', about: 'AutoCAD の交換形式', read: '○', write: '○', place: 'コア', top: true },
  { name: 'DWG', about: 'AutoCAD の図面', read: '○', write: '○', place: 'コア', top: true },
  { name: 'PDF', about: '図面の PDF', read: '―', write: '―', place: 'コア', top: true },
  { name: 'IFC', about: 'BIM・土木のモデル（IFC-SPF・ifcZIP）', read: '○', write: '○', place: 'コア', top: true },
  { name: 'LandXML', about: '線形・地表面などの土木のデータ', read: '○', write: '―', place: 'コア' },
  { name: 'CityGML', about: '都市の 3D モデル', read: '○', write: '―', place: 'コア' },
  { name: '基盤地図情報', about: '国土地理院の地理のデータ（GML）', read: '○', write: '―', place: 'コア' },
  { name: '拡張 DM', about: '測量成果の地理のデータ', read: '○', write: '―', place: 'コア' },
  { name: 'LAS・PLY・XYZ・E57', about: '点群のデータ。LAZ は公開版に含めない', read: '○', write: '―', place: 'コア' },
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
    a: `v${releaseVersion} を公開しています。<a href="/platforms/#packages">言語ごとの配布先と導入手順</a>をご覧ください。ブラウザーでは<a href="/viewer/">図面ビューアー</a>で試せます。対応する形式と操作は<a href="/formats/">対応形式</a>で案内しています。`,
    top: true,
  },
  {
    q: '費用はかかりますか。会社の業務で使えますか。',
    a: 'MIT License または Apache License 2.0 で公開しており、費用はかかりません。社内のシステムや製品に組み込んで使えます。',
    top: true,
  },
  {
    q: '図面を外部のサーバーに送りますか。',
    a: '送りません。図面は、利用者のパソコン・ブラウザー・社内のサーバーの中で読みます。',
    top: true,
  },
  {
    q: '電子納品の SXF や Jw_cad の図面を読めますか。',
    a: '公開版で SXF（SFC・P21・SFZ・P2Z）と JWW を読めます。対応する図形と書き戻しの範囲は<a href="/formats/">対応形式</a>をご覧ください。',
  },
  {
    q: 'どの言語・ソフトから使えますか。',
    a: 'Rust・TypeScript・JavaScript・Python・.NET・C・C++ と CLI・MCP の配布物を公開しています。Java・Swift・Go・VBA などの配布と、Revit・AutoCAD・Excel などでの利用は計画を進めています。詳しくは<a href="/platforms/">対応環境</a>をご覧ください。',
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
