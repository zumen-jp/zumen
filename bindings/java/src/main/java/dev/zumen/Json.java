package dev.zumen;

import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CharsetDecoder;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Function;

/**
 * 小さな JSON の読み書き（RFC 8259）。Java 8 と Android には標準の JSON の道具がないので、包みの中に持つ。
 *
 * <p>読んだ値は、オブジェクトが {@link Map}（欄の順を保つ）、配列が {@link List}、文字列が {@link String}、
 * 小数点と指数のない数で long に収まるものが {@link Long}、ほかの数が {@link Double}、真偽値が {@link Boolean}、
 * null が null になる。
 */
public final class Json {
    /** 入れ子の深さの上限。深すぎる入力でスタックを使い切らないようにする */
    private static final int MAX_DEPTH = 512;

    private Json() {}

    /** UTF-8 のバイト列を読む。読めなければ {@link IllegalArgumentException} */
    public static Object parse(byte[] data) {
        return parse(decode(data));
    }

    /** 文字列を読む。読めなければ {@link IllegalArgumentException} */
    public static Object parse(String text) {
        Reader reader = new Reader(text);
        reader.space();
        Object value = reader.value(0);
        reader.space();
        if (reader.at < text.length()) {
            throw reader.error("値のあとに文字がある");
        }
        return value;
    }

    /** 値を JSON の文字列にする。空白を入れず、ASCII でない文字もそのまま書く */
    public static String write(Object value) {
        StringBuilder out = new StringBuilder();
        write(out, value, 0);
        return out.toString();
    }

    /** 値を JSON の UTF-8 のバイト列にする */
    public static byte[] bytes(Object value) {
        return write(value).getBytes(StandardCharsets.UTF_8);
    }

    /** UTF-8 として読む。正しくない並びがあれば {@link IllegalArgumentException} */
    static String decode(byte[] data) {
        CharsetDecoder decoder = StandardCharsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT);
        try {
            CharBuffer chars = decoder.decode(ByteBuffer.wrap(data));
            return chars.toString();
        } catch (CharacterCodingException e) {
            throw new IllegalArgumentException("UTF-8 として読めない", e);
        }
    }

    // メッセージの型が欄を読むときに使う。形が違えば null にする

    /** 値を read で読む。null なら null */
    public static <T> T read(Object value, Function<Object, T> read) {
        return value == null ? null : read.apply(value);
    }

    /** 値をそのまま返す */
    public static Object value(Object value) {
        return value;
    }

    /** オブジェクトなら、その表。違えば null */
    @SuppressWarnings("unchecked")
    public static Map<String, Object> object(Object value) {
        return value instanceof Map ? (Map<String, Object>) value : null;
    }

    /** 文字列なら、その値。違えば null */
    public static String string(Object value) {
        return value instanceof String ? (String) value : null;
    }

    /** 真偽値なら、その値。違えば null */
    public static Boolean bool(Object value) {
        return value instanceof Boolean ? (Boolean) value : null;
    }

    /** 整数なら、その値。違えば null */
    public static Long integer(Object value) {
        if (value instanceof Long) {
            return (Long) value;
        }
        if (value instanceof Integer || value instanceof Short || value instanceof Byte) {
            return ((Number) value).longValue();
        }
        if (value instanceof Double) {
            double d = (Double) value;
            if (d == Math.rint(d) && Math.abs(d) <= 9007199254740991.0) {
                return (long) d;
            }
        }
        return null;
    }

    /** 数なら、その値。違えば null */
    public static Double number(Object value) {
        return value instanceof Number ? ((Number) value).doubleValue() : null;
    }

    /** 配列なら、要素ごとに read で読んだ列（写し）。違えば null */
    public static <T> List<T> list(Object value, Function<Object, T> read) {
        if (!(value instanceof List)) {
            return null;
        }
        List<?> items = (List<?>) value;
        List<T> out = new ArrayList<T>(items.size());
        for (Object item : items) {
            out.add(read(item, read));
        }
        return out;
    }

    /** オブジェクトなら、値ごとに read で読んだ表（写し）。違えば null */
    public static <T> Map<String, T> map(Object value, Function<Object, T> read) {
        Map<String, Object> object = object(value);
        if (object == null) {
            return null;
        }
        Map<String, T> out = new LinkedHashMap<String, T>();
        for (Map.Entry<String, Object> entry : object.entrySet()) {
            out.put(entry.getKey(), read(entry.getValue(), read));
        }
        return out;
    }

    /** メッセージの型を、包んでいる JSON の値に直す。列と表は中まで直す */
    public static Object unwrap(Object value) {
        if (value instanceof Messages.Message) {
            return ((Messages.Message) value).json();
        }
        if (value instanceof List) {
            List<?> items = (List<?>) value;
            List<Object> out = new ArrayList<Object>(items.size());
            for (Object item : items) {
                out.add(unwrap(item));
            }
            return out;
        }
        if (value instanceof Map) {
            Map<?, ?> object = (Map<?, ?>) value;
            Map<String, Object> out = new LinkedHashMap<String, Object>();
            for (Map.Entry<?, ?> entry : object.entrySet()) {
                out.put(String.valueOf(entry.getKey()), unwrap(entry.getValue()));
            }
            return out;
        }
        return value;
    }

    private static void write(StringBuilder out, Object value, int depth) {
        if (depth > MAX_DEPTH) {
            throw new IllegalArgumentException("入れ子が深すぎる");
        }
        if (value == null) {
            out.append("null");
        } else if (value instanceof Messages.Message) {
            write(out, ((Messages.Message) value).json(), depth);
        } else if (value instanceof String) {
            string(out, (String) value);
        } else if (value instanceof Boolean) {
            out.append(((Boolean) value) ? "true" : "false");
        } else if (value instanceof Double || value instanceof Float) {
            double d = ((Number) value).doubleValue();
            if (Double.isNaN(d) || Double.isInfinite(d)) {
                throw new IllegalArgumentException("有限でない数は JSON に書けない");
            }
            out.append(Double.toString(d));
        } else if (value instanceof Number) {
            out.append(value.toString());
        } else if (value instanceof Map) {
            out.append('{');
            boolean first = true;
            for (Map.Entry<?, ?> entry : ((Map<?, ?>) value).entrySet()) {
                if (!first) {
                    out.append(',');
                }
                first = false;
                string(out, String.valueOf(entry.getKey()));
                out.append(':');
                write(out, entry.getValue(), depth + 1);
            }
            out.append('}');
        } else if (value instanceof Iterable) {
            out.append('[');
            boolean first = true;
            for (Object item : (Iterable<?>) value) {
                if (!first) {
                    out.append(',');
                }
                first = false;
                write(out, item, depth + 1);
            }
            out.append(']');
        } else {
            throw new IllegalArgumentException("JSON に書けない値: " + value.getClass().getName());
        }
    }

    private static void string(StringBuilder out, String text) {
        out.append('"');
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            switch (c) {
                case '"':
                    out.append("\\\"");
                    break;
                case '\\':
                    out.append("\\\\");
                    break;
                case '\b':
                    out.append("\\b");
                    break;
                case '\f':
                    out.append("\\f");
                    break;
                case '\n':
                    out.append("\\n");
                    break;
                case '\r':
                    out.append("\\r");
                    break;
                case '\t':
                    out.append("\\t");
                    break;
                default:
                    if (c < 0x20) {
                        out.append(String.format("\\u%04x", (int) c));
                    } else {
                        out.append(c);
                    }
            }
        }
        out.append('"');
    }

    private static final class Reader {
        final String text;
        int at;

        Reader(String text) {
            this.text = text;
        }

        IllegalArgumentException error(String message) {
            return new IllegalArgumentException("JSON を読めない（" + at + " 文字目）: " + message);
        }

        void space() {
            while (at < text.length()) {
                char c = text.charAt(at);
                if (c != ' ' && c != '\t' && c != '\n' && c != '\r') {
                    return;
                }
                at++;
            }
        }

        boolean literal(String word) {
            if (text.startsWith(word, at)) {
                at += word.length();
                return true;
            }
            return false;
        }

        Object value(int depth) {
            if (depth > MAX_DEPTH) {
                throw error("入れ子が深すぎる");
            }
            if (at >= text.length()) {
                throw error("値がない");
            }
            char c = text.charAt(at);
            if (c == '{') {
                return object(depth);
            }
            if (c == '[') {
                return array(depth);
            }
            if (c == '"') {
                return string();
            }
            if (c == '-' || (c >= '0' && c <= '9')) {
                return number();
            }
            if (literal("true")) {
                return Boolean.TRUE;
            }
            if (literal("false")) {
                return Boolean.FALSE;
            }
            if (literal("null")) {
                return null;
            }
            throw error("値でない文字");
        }

        Map<String, Object> object(int depth) {
            at++;
            Map<String, Object> out = new LinkedHashMap<String, Object>();
            space();
            if (at < text.length() && text.charAt(at) == '}') {
                at++;
                return out;
            }
            while (true) {
                space();
                if (at >= text.length() || text.charAt(at) != '"') {
                    throw error("欄の名前がない");
                }
                String key = string();
                space();
                if (at >= text.length() || text.charAt(at) != ':') {
                    throw error(": がない");
                }
                at++;
                space();
                out.put(key, value(depth + 1));
                space();
                if (at >= text.length()) {
                    throw error("オブジェクトが閉じていない");
                }
                char c = text.charAt(at++);
                if (c == '}') {
                    return out;
                }
                if (c != ',') {
                    throw error(", か } がない");
                }
            }
        }

        List<Object> array(int depth) {
            at++;
            List<Object> out = new ArrayList<Object>();
            space();
            if (at < text.length() && text.charAt(at) == ']') {
                at++;
                return out;
            }
            while (true) {
                space();
                out.add(value(depth + 1));
                space();
                if (at >= text.length()) {
                    throw error("配列が閉じていない");
                }
                char c = text.charAt(at++);
                if (c == ']') {
                    return Collections.unmodifiableList(out).isEmpty() ? out : out;
                }
                if (c != ',') {
                    throw error(", か ] がない");
                }
            }
        }

        String string() {
            at++;
            StringBuilder out = new StringBuilder();
            while (true) {
                if (at >= text.length()) {
                    throw error("文字列が閉じていない");
                }
                char c = text.charAt(at++);
                if (c == '"') {
                    return out.toString();
                }
                if (c < 0x20) {
                    throw error("文字列に制御文字がある");
                }
                if (c != '\\') {
                    out.append(c);
                    continue;
                }
                if (at >= text.length()) {
                    throw error("文字列が閉じていない");
                }
                char e = text.charAt(at++);
                switch (e) {
                    case '"':
                    case '\\':
                    case '/':
                        out.append(e);
                        break;
                    case 'b':
                        out.append('\b');
                        break;
                    case 'f':
                        out.append('\f');
                        break;
                    case 'n':
                        out.append('\n');
                        break;
                    case 'r':
                        out.append('\r');
                        break;
                    case 't':
                        out.append('\t');
                        break;
                    case 'u':
                        if (at + 4 > text.length()) {
                            throw error("\\u のあとに 4 桁がない");
                        }
                        int code = 0;
                        for (int i = 0; i < 4; i++) {
                            int digit = Character.digit(text.charAt(at++), 16);
                            if (digit < 0) {
                                throw error("\\u のあとが 16 進でない");
                            }
                            code = code * 16 + digit;
                        }
                        out.append((char) code);
                        break;
                    default:
                        throw error("知らないエスケープ");
                }
            }
        }

        Object number() {
            int start = at;
            boolean integral = true;
            if (text.charAt(at) == '-') {
                at++;
            }
            if (at >= text.length()) {
                throw error("数が途中で終わる");
            }
            if (text.charAt(at) == '0') {
                at++;
            } else if (digits() == 0) {
                throw error("数の桁がない");
            }
            if (at < text.length() && text.charAt(at) == '.') {
                integral = false;
                at++;
                if (digits() == 0) {
                    throw error("小数点のあとに桁がない");
                }
            }
            if (at < text.length() && (text.charAt(at) == 'e' || text.charAt(at) == 'E')) {
                integral = false;
                at++;
                if (at < text.length() && (text.charAt(at) == '+' || text.charAt(at) == '-')) {
                    at++;
                }
                if (digits() == 0) {
                    throw error("指数の桁がない");
                }
            }
            String literal = text.substring(start, at);
            if (integral) {
                try {
                    return Long.parseLong(literal);
                } catch (NumberFormatException e) {
                    // long に収まらない整数は、小数で読む
                }
            }
            return Double.parseDouble(literal);
        }

        int digits() {
            int start = at;
            while (at < text.length() && text.charAt(at) >= '0' && text.charAt(at) <= '9') {
                at++;
            }
            return at - start;
        }
    }
}
