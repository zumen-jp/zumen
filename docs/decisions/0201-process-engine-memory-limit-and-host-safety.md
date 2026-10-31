# 0201: 子プロセスのメモリの上限は Linux の setrlimit と Windows のジョブ・オブジェクトで付け、Windows でもライブラリの場所から CLI を探し、Node.js の process はまだ作らない

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- [0013](0013-boundary-in-four-forms.md) は、`process` の子プロセスのメモリに OS の仕組み（Windows のジョブ・オブジェクトなど）で上限を付けるとした。上限の値は決めていない。[0200](0200-cli-http-process-engine-and-mcp-resources.md) は、メモリの上限・Windows でライブラリの場所を探すこと・Node.js の包みの `process`・画像の大きさの上限を残した。
- 予算の既定は、`input_bytes` と `decoded_bytes` がそれぞれ 2 GiB（[0010](0010-budget-by-work.md)）。
- 子プロセスへの書き込みは、終わった子プロセスかを先に確かめてから書いていた。確かめたあとに子プロセスが終わると、SIGPIPE が届き、無視していないホスト（C や C++ のホストの既定）は止まる。
- OS ごとの違いを、2026-10-31 に手元とカーネルのソースで確かめた。
  - macOS は `RLIMIT_AS` を受け付けない（`setrlimit` が EINVAL）。`RLIMIT_DATA` は `brk` にだけ効き、`mmap` で確保するメモリには効かない。
  - パイプへの書き込みの SIGPIPE を、Linux と FreeBSD は書いたスレッドに、macOS はプロセスに送る。macOS には書き口ごとに SIGPIPE を止める `F_SETNOSIGPIPE` がある。
- JavaScript の包みは同期の API で、wasm のモジュールを呼ぶ（[0194](0194-typescript-javascript-wrapper.md)）。Node.js で子プロセスと同期でやり取りするには、呼び出しごとに子プロセスを起こす（`spawnSync`）か、Worker と `Atomics.wait` で待つしかない。

## 選択肢

### メモリの上限を付ける所

1. **ライブラリが、子プロセスを起こすときに付ける。** Unix は exec の前に子の側で `setrlimit`（`pre_exec`）、Windows は起こしてすぐにジョブ・オブジェクトへ入れる。`process_path` が指す実行ファイルが何でも効く。
2. **CLI が、引数で受けた上限を自分に付ける。** ライブラリは `unsafe` を持たずに済むが、CLI が従わなければ効かない。Windows でホストが落ちたとき子プロセスを残さない（ジョブを閉じると止まる）ことも得られない。

### SIGPIPE を防ぐ方法

1. **書く前後でホストのスレッドの SIGPIPE を止め、保留の SIGPIPE を `sigwait` で取り除く。** macOS では別のスレッドに届くので防げない。
2. **書くスレッドを分け、そのスレッドで防ぐ。** Linux と FreeBSD は、そのスレッドで SIGPIPE を止める。macOS は書き口に `F_SETNOSIGPIPE` を付ける。ホストのスレッドとシグナルの扱いを変えない。

### Node.js の包みの `process`

1. **同期の API のまま、Worker と `Atomics.wait` で子プロセスを待つ。** Worker と SharedArrayBuffer の仕組みが包みに入り、依存のない素の ESM という形（[0194](0194-typescript-javascript-wrapper.md)）から大きく離れる。
2. **非同期のセッション（`Promise` を返す `push`・`call`・`take`）を足す。** 公開の API が 2 つの形になる。
3. **まだ作らない。** Node.js からは、CLI の `serve --stdio`（公開の約束の JSON-RPC）を直接使える。

## 決定

メモリの上限は 1、SIGPIPE は 2、Node.js は 3 を選ぶ。依存は足さず、OS の関数は `extern "C"`・`extern "system"` で宣言する。

### メモリの上限

- 上限は 8 GiB。予算の既定の `input_bytes` と `decoded_bytes` の和（4 GiB）の 2 倍にし、既定の予算で読める図面を上限で落とさないようにする。暴走したときに、ホストの機械のメモリを使い切らないためのもの。
- Linux と Android: 子プロセスが exec する前に、`setrlimit` でアドレス空間（`RLIMIT_AS`）の上限を下げる。今の上限が 8 GiB より低ければ、そのまま。
- Windows: 子プロセスを起こしてすぐ、`JOB_OBJECT_LIMIT_PROCESS_MEMORY`（確定したメモリ）と `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` を付けたジョブ・オブジェクトに入れる。子プロセスは標準入力から要求を読むまで何もしないので、入れる前に使うことはない。入れられなければ、子プロセスを止めて `zm_session_new` は `failed` を返す。ジョブを閉じると子プロセスも止まるので、ホストが落ちても子プロセスは残らない。
- macOS: 付けられる上限がないので、付けない。
- 32 bit のホストで表せない値は、上限なしと同じにする。
- 上限を変える設定（`session_options` の欄）は、まだ足さない。要るようになったら、スキーマに足す。

### Windows でライブラリの場所を探す

- `GetModuleHandleExW`（`FROM_ADDRESS`、参照の数を変えない）でライブラリの関数の番地を含むモジュールを求め、`GetModuleFileNameW` でパスを読み、同じフォルダーの `zumen.exe` を起こす。パスが入りきらなければ、32768 文字まで領域を広げて読み直す。

### SIGPIPE

- 子プロセスの標準入力へは、書くスレッドで書く。ホストのスレッドは行を渡し、書けたかどうかを待つ。
- Linux・Android・FreeBSD は、書くスレッドで SIGPIPE を止める（`pthread_sigmask`）。保留の SIGPIPE は、スレッドと一緒に消える。macOS は、書き口に `F_SETNOSIGPIPE` を付ける。ほかの Unix は、これまでどおり先に終わったかを確かめるだけにする。Windows に SIGPIPE はない。
- 終わった子プロセスかを先に確かめるのは、残す。

### Node.js の包みの `process`

- まだ作らない。同期の API に Worker を入れるか、非同期の API を足すかは、公開の API の形を決めることなので、Node.js で子プロセスに分けたい使い方が出てから決める。それまでは、CLI の `serve --stdio` を `child_process` で起こし、同じ JSON-RPC を送れば同じ結果になる。
- 包みの `Zumen.open({ engine: "process" })` は、これまでどおり wasm のモジュールが `failed` を返す。

### MCP の画像の大きさの上限

- 作らない。画像を返す op（blob の種類 `image` を作るもの）がまだなく、それを作る issue もない。描画の op を足すときに、[0200](0200-cli-http-process-engine-and-mcp-resources.md) のとおり、要求に画素の数の上限を持たせ、MCP では `image` の内容として、その上限と埋め込みの上限（16 MiB）で返す。

## 影響

- `crates/zumen-abi/src/process.rs` に、OS ごとの宣言（`unsafe` を許すモジュール）が増える。Windows の部分は、Windows の target の clippy で確かめ、動かす試験は Windows の CI で流れる。
- 試験: Linux では、上限を付けた `/bin/sh` の `ulimit -v` が 8 GiB を返すことを確かめる。SIGPIPE は、試験のプロセスの SIGPIPE を既定の扱いに戻し、終わった子プロセスへ書いても止まらないことを確かめる（Linux と macOS）。Windows では、ライブラリのそばの `zumen.exe` を指すことを確かめる。
- macOS の子プロセスにはメモリの上限がない。
- 残り: Node.js の包みの `process`（使い方が出てから）、画像の大きさの上限（描画の op と一緒に）、上限を変える設定（要るようになったら）。

## 参照

- [#357](https://github.com/zumen-jp/zumen/issues/357)
- [0010](0010-budget-by-work.md)、[0013](0013-boundary-in-four-forms.md)、[0194](0194-typescript-javascript-wrapper.md)、[0197](0197-cli-and-mcp-server.md)、[0200](0200-cli-http-process-engine-and-mcp-resources.md)
- [setrlimit（POSIX）](https://pubs.opengroup.org/onlinepubs/9799919799/functions/setrlimit.html)、[JOBOBJECT_EXTENDED_LIMIT_INFORMATION](https://learn.microsoft.com/windows/win32/api/winnt/ns-winnt-jobobject_extended_limit_information)、[GetModuleHandleExW](https://learn.microsoft.com/windows/win32/api/libloaderapi/nf-libloaderapi-getmodulehandleexw)、[GetModuleFileNameW](https://learn.microsoft.com/windows/win32/api/libloaderapi/nf-libloaderapi-getmodulefilenamew)
