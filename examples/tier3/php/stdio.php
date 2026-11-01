<?php
// zumen を、PHP から CLI の `serve --stdio`（JSON-RPC）で呼ぶサンプル（#404、決定 0006・0013）。
//
// 使い方: php examples/tier3/php/stdio.php <図面のファイル> <CLI を起こすコマンド>...
//
// 例: php stdio.php a.dxf target/release/zumen serve --stdio
//     php stdio.php a.dxf wasmtime run zumen.wasm serve --stdio（WASI のランタイムの上の CLI）
//
// 図面を開き、形式と図形の数を書く。

if ($argc < 3) {
    fwrite(STDERR, "使い方: php stdio.php <図面のファイル> <CLI を起こすコマンド>...\n");
    exit(2);
}
$drawing = $argv[1];
$command = array_slice($argv, 2);
$process = proc_open($command, [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => STDERR], $pipes);
if (!is_resource($process)) {
    fwrite(STDERR, "CLI を起こせない\n");
    exit(1);
}

$next = 1;
// 要求を 1 行で送り、同じ id の答えを待つ。id のない行（進み具合の通知）は読み飛ばす
function rpc($pipes, &$next, $method, $params)
{
    $id = $next++;
    $line = json_encode(['jsonrpc' => '2.0', 'id' => $id, 'method' => $method, 'params' => $params], JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    fwrite($pipes[0], $line . "\n");
    fflush($pipes[0]);
    while (($reply = fgets($pipes[1])) !== false) {
        $message = json_decode($reply, true);
        if (($message['id'] ?? null) !== $id) {
            continue;
        }
        if (isset($message['error'])) {
            throw new RuntimeException("$method が誤りを返した: " . json_encode($message['error']));
        }
        return $message['result'];
    }
    throw new RuntimeException("$method の答えがない");
}

// 境界の要求を送り、応答の JSON を読む
function call($pipes, &$next, $request)
{
    $result = rpc($pipes, $next, 'call', ['request' => $request]);
    if ($result['status'] !== 'ok') {
        throw new RuntimeException("{$request['op']} が {$result['status']} を返した: " . ($result['response'] ?? ''));
    }
    return json_decode($result['response'], true);
}

$name = basename($drawing);
$pushed = rpc($pipes, $next, 'push', ['name' => $name, 'base64' => base64_encode(file_get_contents($drawing))]);
if ($pushed['status'] !== 'ok') {
    throw new RuntimeException("push が {$pushed['status']} を返した");
}
$opened = call($pipes, $next, ['v' => 1, 'op' => 'open', 'input' => $name]);
$entities = call($pipes, $next, ['v' => 1, 'op' => 'entities', 'doc' => $name]);
echo "format {$opened['result']['format']}\n";
echo "entities {$entities['result']['total']}\n";

fclose($pipes[0]);
fclose($pipes[1]);
proc_close($process);
