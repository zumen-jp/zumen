package dev.zumen;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStreamWriter;
import java.io.PrintStream;
import java.io.UnsupportedEncodingException;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;
import java.util.Map;

/**
 * 適合試験を、Java の包み（bindings/java）を通して流す（決定 0006・0224）。
 *
 * <p>使い方: {@code java -cp <クラスの道筋> dev.zumen.Conformance <ネイティブのライブラリ> [リポジトリの根]}
 *
 * <p>bindings/conformance/cases/*.json を、包みのセッションのメソッドで流し、状態と応答の値が期待どおりで、ハッシュの
 * 一覧が hashes.txt と一致することを確かめる。シーンの blob は、包みの読み手で読み、応答の数と合うかを確かめる。
 * 手順の読み方と応答の確かめ方は、Python の側（bindings/python/tests/conformance.py）と同じ規則で書く。
 * Android のエミュレーターでも、同じクラスを dalvikvm で流す。
 */
public final class Conformance {
    /** 手順の種類。手順は、どれか 1 つの欄を持つ */
    private static final String[] ACTIONS = {"session", "push", "call", "call_text", "call_hex", "take"};

    /** JSON Pointer で引けなかった印 */
    private static final Object MISSING = new Object();

    private final File repository;
    private final File conformance;
    private final List<String> failures = new ArrayList<String>();
    private final List<String> hashes = new ArrayList<String>();
    private int scenes;

    private Conformance(File repository) {
        this.repository = repository;
        this.conformance = new File(new File(repository, "bindings"), "conformance");
    }

    /** 境界の関数の結果。状態の名前と、書かれたバイト列 */
    private static final class Outcome {
        final String status;
        final byte[] data;

        Outcome(String status, byte[] data) {
            this.status = status;
            this.data = data;
        }
    }

    private interface Call {
        byte[] run();
    }

    /** 境界の関数を呼び、状態の名前と、書かれたバイト列を返す */
    private static Outcome attempt(Call call) {
        try {
            return new Outcome("ok", call.run());
        } catch (ZumenException e) {
            return new Outcome(e.getStatus(), e.getData());
        }
    }

    static byte[] decodeHex(String text) {
        StringBuilder digits = new StringBuilder();
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            if (!Character.isWhitespace(c)) {
                digits.append(c);
            }
        }
        if (digits.length() % 2 != 0) {
            throw new IllegalArgumentException("16 進として読めない: " + text);
        }
        byte[] out = new byte[digits.length() / 2];
        for (int i = 0; i < out.length; i++) {
            int high = Character.digit(digits.charAt(2 * i), 16);
            int low = Character.digit(digits.charAt(2 * i + 1), 16);
            if (high < 0 || low < 0) {
                throw new IllegalArgumentException("16 進として読めない: " + text);
            }
            out[i] = (byte) (high * 16 + low);
        }
        return out;
    }

    /** JSON Pointer で値を引く。なければ MISSING */
    static Object pointer(Object value, String path) {
        if (path.isEmpty()) {
            return value;
        }
        Object current = value;
        for (String raw : path.substring(1).split("/", -1)) {
            String token = raw.replace("~1", "/").replace("~0", "~");
            if (current instanceof List) {
                List<?> items = (List<?>) current;
                if (!token.matches("0|[1-9][0-9]*") || token.length() > 9 || Integer.parseInt(token) >= items.size()) {
                    return MISSING;
                }
                current = items.get(Integer.parseInt(token));
            } else if (current instanceof Map && ((Map<?, ?>) current).containsKey(token)) {
                current = ((Map<?, ?>) current).get(token);
            } else {
                return MISSING;
            }
        }
        return current;
    }

    /** JSON の値として等しいか。真偽値と数は別にする */
    static boolean same(Object a, Object b) {
        if (a instanceof Boolean || b instanceof Boolean) {
            return a instanceof Boolean && b instanceof Boolean && a.equals(b);
        }
        if (a instanceof List || b instanceof List) {
            if (!(a instanceof List && b instanceof List) || ((List<?>) a).size() != ((List<?>) b).size()) {
                return false;
            }
            Iterator<?> x = ((List<?>) a).iterator();
            Iterator<?> y = ((List<?>) b).iterator();
            while (x.hasNext()) {
                if (!same(x.next(), y.next())) {
                    return false;
                }
            }
            return true;
        }
        if (a instanceof Map || b instanceof Map) {
            if (!(a instanceof Map && b instanceof Map) || !((Map<?, ?>) a).keySet().equals(((Map<?, ?>) b).keySet())) {
                return false;
            }
            for (Map.Entry<?, ?> entry : ((Map<?, ?>) a).entrySet()) {
                if (!same(entry.getValue(), ((Map<?, ?>) b).get(entry.getKey()))) {
                    return false;
                }
            }
            return true;
        }
        if (a instanceof Number && b instanceof Number) {
            if (a instanceof Long && b instanceof Long) {
                return a.equals(b);
            }
            return ((Number) a).doubleValue() == ((Number) b).doubleValue();
        }
        return a == null ? b == null : b != null && a.getClass() == b.getClass() && a.equals(b);
    }

    static byte[] readFile(File file) throws IOException {
        InputStream in = new FileInputStream(file);
        try {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buffer = new byte[65536];
            int n;
            while ((n = in.read(buffer)) > 0) {
                out.write(buffer, 0, n);
            }
            return out.toByteArray();
        } finally {
            in.close();
        }
    }

    static String hex(byte[] data) {
        StringBuilder out = new StringBuilder();
        for (byte b : data) {
            out.append(String.format("%02x", b & 0xff));
        }
        return out.toString();
    }

    static String sha256(byte[] data) {
        try {
            return hex(MessageDigest.getInstance("SHA-256").digest(data));
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException(e);
        }
    }

    static byte[] utf8(String text) {
        return text.getBytes(StandardCharsets.UTF_8);
    }

    /** session の設定か、call の要求のバイト列。値は JSON に直し、文字列はそのまま渡す */
    static byte[] bytesOf(Object input, String valueKey, String textKey) {
        Map<String, Object> object = Json.object(input);
        if (object == null) {
            return new byte[0];
        }
        if (object.containsKey(valueKey)) {
            return Json.bytes(object.get(valueKey));
        }
        if (object.containsKey(textKey)) {
            return utf8((String) object.get(textKey));
        }
        return new byte[0];
    }

    /** 公開サンプル（決定 0171）を読む。道筋はリポジトリの根から書き、samples/ の中に限る */
    byte[] readSample(String file) throws IOException {
        String[] parts = file.split("/", -1);
        boolean valid = parts.length >= 2 && parts[0].equals("samples");
        for (int i = 1; i < parts.length; i++) {
            String part = parts[i];
            if (part.isEmpty() || part.equals(".") || part.equals("..") || part.contains("\\")) {
                valid = false;
            }
        }
        if (!valid) {
            throw new IllegalArgumentException("push の file は samples/ の中のファイルにする: " + file);
        }
        File path = repository;
        for (String part : parts) {
            path = new File(path, part);
        }
        return readFile(path);
    }

    /** call・call_text・call_hex の手順の、要求のバイト列 */
    static byte[] requestOf(String action, Object input) {
        if (action.equals("call_text")) {
            return utf8((String) input);
        }
        if (action.equals("call_hex")) {
            return decodeHex((String) input);
        }
        return Json.bytes(input);
    }

    /** 応答の値を、手順の expect と absent で確かめる */
    void checkResponse(String at, Map<String, Object> step, Object response) {
        Map<String, Object> expect = Json.object(step.get("expect"));
        if (expect != null) {
            for (Map.Entry<String, Object> entry : expect.entrySet()) {
                Object found = pointer(response, entry.getKey());
                if (found == MISSING) {
                    failures.add(at + ": " + entry.getKey() + " がない");
                } else if (!same(found, entry.getValue())) {
                    failures.add(at + ": " + entry.getKey() + " が " + Json.write(found) + "（期待は "
                            + Json.write(entry.getValue()) + "）");
                }
            }
        }
        List<String> absent = Json.list(step.get("absent"), Json::string);
        if (absent != null) {
            for (String path : absent) {
                if (pointer(response, path) != MISSING) {
                    failures.add(at + ": " + path + " があってはいけない");
                }
            }
        }
    }

    /** シーンの blob を読み手で読み、応答の数と合うかを確かめる */
    void checkScene(String at, byte[] blob, Map<String, Object> result) {
        Scene scene;
        try {
            scene = Scene.read(blob);
        } catch (IllegalArgumentException e) {
            failures.add(at + ": シーンを読めない: " + e.getMessage());
            return;
        }
        Map<String, Object> counts = Json.object(result.get("counts"));
        long chunks = Json.integer(counts.get("chunks"));
        Object[][] checks = {
            {"大版", (long) scene.major, (long) Constants.SCENE_MAJOR},
            {"区画の数", scene.sections, (long) Constants.SCENE_SECTIONS.length},
            {"塊の数", scene.chunks, chunks},
            {"chunk_origins の行", (long) scene.rows("chunk_origins"), chunks},
            {"text_xy の行", (long) scene.rows("text_xy"), Json.integer(counts.get("texts"))},
            {"point_xy の行", (long) scene.rows("point_xy"), Json.integer(counts.get("points"))},
            {"glyph_xy の行", (long) scene.rows("glyph_xy"), Json.integer(counts.get("glyphs"))},
            {
                "line_xy と face_xy の行",
                (long) (scene.rows("line_xy") + scene.rows("face_xy")),
                Json.integer(counts.get("vertices"))
            },
        };
        // 文字の文字列は、strings の区画から引ける
        int[] strings = scene.u32("text_strings");
        for (int row = 0; row + 1 < strings.length; row += 2) {
            try {
                scene.text(strings[row] & 0xffffffffL, strings[row + 1] & 0xffffffffL);
            } catch (RuntimeException e) {
                failures.add(at + ": text_strings の行 " + (row / 2) + " を引けない: " + e.getMessage());
            }
        }
        for (Object[] check : checks) {
            if (!check[1].equals(check[2])) {
                failures.add(at + ": シーンの" + check[0] + "が " + check[1] + "（期待は " + check[2] + "）");
            }
        }
    }

    /** 試験の組のファイル。名前の順 */
    List<File> caseFiles() {
        File[] files = new File(conformance, "cases").listFiles();
        List<File> out = new ArrayList<File>();
        if (files != null) {
            for (File file : files) {
                if (file.getName().endsWith(".json")) {
                    out.add(file);
                }
            }
        }
        Collections.sort(out, (a, b) -> a.getName().compareTo(b.getName()));
        return out;
    }

    int run(final Zumen zumen) throws IOException {
        List<File> files = caseFiles();
        for (File file : files) {
            String name = file.getName().substring(0, file.getName().length() - ".json".length());
            List<Object> steps = Json.list(Json.object(Json.parse(readFile(file))).get("steps"), Json::value);
            Session session = null;
            // 直前の応答が返したシーンの blob の番号と結果
            Map<String, Object> lastScene = null;
            long lastBlob = -1;
            for (int index = 0; index < steps.size(); index++) {
                String at = name + " の手順 " + index;
                Map<String, Object> step = Json.object(steps.get(index));
                String action = null;
                for (String candidate : ACTIONS) {
                    if (step.containsKey(candidate)) {
                        action = candidate;
                        break;
                    }
                }
                if (action == null) {
                    failures.add(at + ": 手順の種類がない");
                    continue;
                }
                String expected = step.containsKey("status") ? (String) step.get("status") : "ok";
                final Object input = step.get(action);
                if (!action.equals("session") && session == null) {
                    session = zumen.open();
                }
                final Session current = session;
                Outcome outcome;
                String kind = null;
                if (action.equals("session")) {
                    if (session != null) {
                        session.close();
                    }
                    final Session[] opened = new Session[1];
                    outcome = attempt(() -> {
                        opened[0] = zumen.open(bytesOf(input, "options", "options_text"));
                        return null;
                    });
                    if (outcome.status.equals("ok")) {
                        session = opened[0];
                    } else {
                        // 作れなかったら、閉じたセッションで続ける。境界は、空のセッションへの呼び出しを invalid にする
                        session = zumen.open();
                        session.close();
                    }
                } else if (action.equals("push")) {
                    Map<String, Object> push = Json.object(input);
                    final byte[] pushName = push.containsKey("name_hex")
                            ? decodeHex((String) push.get("name_hex"))
                            : utf8((String) push.get("name"));
                    final byte[] data;
                    if (push.containsKey("hex")) {
                        data = decodeHex((String) push.get("hex"));
                    } else if (push.containsKey("file")) {
                        data = readSample((String) push.get("file"));
                    } else {
                        data = utf8(push.containsKey("text") ? (String) push.get("text") : "");
                    }
                    outcome = attempt(() -> {
                        current.push(pushName, data);
                        return null;
                    });
                } else if (action.equals("take")) {
                    final int blob = Json.integer(input).intValue();
                    outcome = attempt(() -> current.take(blob));
                    kind = outcome.data != null ? "blob" : null;
                    if (outcome.status.equals("ok") && lastScene != null && lastBlob == blob) {
                        checkScene(at, outcome.data, lastScene);
                        scenes++;
                    }
                } else {
                    final byte[] request = requestOf(action, input);
                    outcome = attempt(() -> current.callBytes(request));
                    kind = outcome.data != null ? "response" : null;
                }
                if (!outcome.status.equals(expected)) {
                    failures.add(at + ": 状態が " + outcome.status + "（期待は " + expected + "）");
                }
                if (kind == null) {
                    continue;
                }
                hashes.add(name + " " + index + " " + kind + " " + sha256(outcome.data));
                if (!kind.equals("response")) {
                    continue;
                }
                Map<String, Object> response = Json.object(Json.parse(outcome.data));
                checkResponse(at, step, response);
                lastScene = null;
                lastBlob = -1;
                Map<String, Object> result = Json.object(response.get("result"));
                List<Object> blobs = Json.list(response.get("blobs"), Json::value);
                if (blobs != null && result != null) {
                    for (Object entry : blobs) {
                        Map<String, Object> blob = Json.object(entry);
                        if (blob != null && "scene".equals(blob.get("kind"))) {
                            lastScene = result;
                            lastBlob = Json.integer(blob.get("id"));
                            break;
                        }
                    }
                }
            }
            // 動いている呼び出しがないので、何もしない
            session.cancel();
            session.close();
        }

        compareHashes();
        checkTypedCalls(zumen);

        String summary = "包みを通した試験の組 " + files.size() + " 件、ハッシュ " + hashes.size() + " 件、読んだシーン "
                + scenes + " 件";
        System.out.println(summary);
        String stepSummary = System.getenv("GITHUB_STEP_SUMMARY");
        if (stepSummary != null && !stepSummary.isEmpty()) {
            Writer writer = new OutputStreamWriter(new FileOutputStream(stepSummary, true), StandardCharsets.UTF_8);
            try {
                writer.write("### Java の包みの適合試験（" + System.getProperty("java.vm.name") + " "
                        + System.getProperty("java.version") + "）\n\n" + summary + "\n");
            } finally {
                writer.close();
            }
        }
        if (scenes == 0) {
            failures.add("シーンの blob を 1 つも読んでいない");
        }
        if (!failures.isEmpty()) {
            System.err.println("期待と違った:");
            for (String failure : failures) {
                System.err.println(failure);
            }
            return 1;
        }
        System.out.println("ネイティブと同じ一覧になった");
        return 0;
    }

    /** ハッシュの一覧が hashes.txt と一致するかを確かめる */
    void compareHashes() throws IOException {
        String text = new String(readFile(new File(conformance, "hashes.txt")), StandardCharsets.UTF_8);
        List<String> expected = new ArrayList<String>();
        for (String line : text.split("\n", -1)) {
            if (!line.isEmpty() && !line.startsWith("#")) {
                expected.add(line);
            }
        }
        if (!hashes.equals(expected)) {
            StringBuilder message = new StringBuilder("ハッシュの一覧が hashes.txt と合わない:");
            for (String line : expected) {
                if (!hashes.contains(line)) {
                    message.append("\n- ").append(line);
                }
            }
            for (String line : hashes) {
                if (!expected.contains(line)) {
                    message.append("\n+ ").append(line);
                }
            }
            failures.add(message.toString());
        }
    }

    /** 型を渡す call と、失敗した call と、閉じたあとの呼び出し */
    void checkTypedCalls(Zumen zumen) {
        final Session session = zumen.open(new Messages.SessionOptions().withV((long) Constants.SCHEMA_MAJOR));
        try {
            Messages.Response response =
                    session.call(new Messages.CapabilitiesRequest().withV((long) Constants.SCHEMA_MAJOR));
            Messages.CapabilitiesResult result = response.result(Messages.CapabilitiesResult::of);
            if (!Long.valueOf(Constants.SCHEMA_MAJOR).equals(response.getV())
                    || result == null
                    || result.getPackage() == null) {
                failures.add("call の応答が capabilities の形でない: " + response);
            }
            try {
                session.call(new Messages.OpenRequest()
                        .withV((long) Constants.SCHEMA_MAJOR)
                        .withInput("none.dxf"));
                failures.add("ない入力を開く call が失敗しない");
            } catch (ZumenException e) {
                Messages.Response failed = e.getResponse();
                if (!e.getStatus().equals("failed")
                        || failed == null
                        || failed.getDiagnostics() == null
                        || failed.getDiagnostics().isEmpty()) {
                    failures.add("ない入力を開く call の失敗が、診断のある failed でない: " + e.getMessage());
                }
            }
            // ほかのスレッドから cancel を呼べる（動いている呼び出しがなければ、何もしない）
            Thread thread = new Thread(session::cancel);
            thread.start();
            try {
                thread.join();
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            }
        } finally {
            session.close();
        }
        Outcome outcome = attempt(() -> session.callBytes(
                Json.bytes(new Messages.CapabilitiesRequest().withV((long) Constants.SCHEMA_MAJOR))));
        if (!session.isClosed() || !outcome.status.equals("invalid")) {
            failures.add("閉じたあとの call が " + outcome.status + "（期待は invalid）");
        }
    }

    /** 渡されなければ、今の場所から上にたどって、bindings/conformance のあるフォルダーを根にする */
    static File findRepository(String[] args) {
        if (args.length >= 2) {
            return new File(args[1]).getAbsoluteFile();
        }
        File current = new File("").getAbsoluteFile();
        while (current != null) {
            if (new File(new File(new File(current, "bindings"), "conformance"), "hashes.txt").isFile()) {
                return current;
            }
            current = current.getParentFile();
        }
        throw new IllegalStateException("リポジトリの根が見つからない。2 つめの引数で渡す");
    }

    public static void main(String[] args) throws IOException {
        // Windows の端末の既定のコード・ページでは日本語を書けないので、UTF-8 で書く
        try {
            System.setOut(new PrintStream(System.out, true, "UTF-8"));
            System.setErr(new PrintStream(System.err, true, "UTF-8"));
        } catch (UnsupportedEncodingException e) {
            throw new IllegalStateException(e);
        }
        if (args.length < 1 || args.length > 2) {
            System.err.println("使い方: java dev.zumen.Conformance <ネイティブのライブラリ> [リポジトリの根]");
            System.exit(2);
        }
        Zumen zumen = Zumen.load(args[0]);
        System.exit(new Conformance(findRepository(args)).run(zumen));
    }

}
