package dev.zumen;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * 読んだシーンの blob（決定 0137・0224）。配置は schema/scene.toml にあり、区画の定義は {@link Constants} に生成する。
 *
 * <p>見出しは {@code ZMSC}、大版（u16）、小版（u16）、塊の数（u32）、区画の数（u32）で、続けて区画ごとに始まりと長さ
 * （u32、バイト）を置く。数はリトルエンディアン。知らない区画（新しい小版で足したもの）は読み飛ばし、ない区画（古い小版）は
 * 空の列にする。大版が違えば読まない。
 */
public final class Scene {
    private static final int HEADER = 16;
    private static final int ENTRY = 8;

    /** 大版 */
    public final int major;
    /** 小版 */
    public final int minor;
    /** 塊の数 */
    public final long chunks;
    /** blob にある区画の数。この包みが知らない区画も数える */
    public final long sections;

    private final Map<String, ByteBuffer> columns;

    private Scene(int major, int minor, long chunks, long sections, Map<String, ByteBuffer> columns) {
        this.major = major;
        this.minor = minor;
        this.chunks = chunks;
        this.sections = sections;
        this.columns = columns;
    }

    /**
     * シーンの blob を読む。列は blob のバイト列を共有するので、blob を書き換えない
     *
     * @throws IllegalArgumentException シーンの blob でないか、大版が違うか、区画が blob に収まらないとき
     */
    public static Scene read(byte[] blob) {
        ByteBuffer view = ByteBuffer.wrap(blob).order(ByteOrder.LITTLE_ENDIAN);
        if (blob.length < HEADER
                || blob[0] != 'Z'
                || blob[1] != 'M'
                || blob[2] != 'S'
                || blob[3] != 'C') {
            throw new IllegalArgumentException("シーンの blob でない");
        }
        int major = view.getShort(4) & 0xffff;
        int minor = view.getShort(6) & 0xffff;
        long chunks = view.getInt(8) & 0xffffffffL;
        long sections = view.getInt(12) & 0xffffffffL;
        if (major != Constants.SCENE_MAJOR) {
            throw new IllegalArgumentException(
                    "シーンの大版が " + major + "（この包みは " + Constants.SCENE_MAJOR + "）");
        }
        if (HEADER + ENTRY * sections > blob.length) {
            throw new IllegalArgumentException("区画の表が blob に収まらない");
        }
        Map<String, ByteBuffer> columns = new LinkedHashMap<String, ByteBuffer>();
        for (int index = 0; index < Constants.SCENE_SECTIONS.length; index++) {
            Constants.SceneSection section = Constants.SCENE_SECTIONS[index];
            if (index >= sections) {
                columns.put(section.name, ByteBuffer.allocate(0).order(ByteOrder.LITTLE_ENDIAN));
                continue;
            }
            long start = view.getInt(HEADER + ENTRY * index) & 0xffffffffL;
            long length = view.getInt(HEADER + ENTRY * index + 4) & 0xffffffffL;
            long row = (long) section.type.size * section.width;
            if (start + length > blob.length || length % row != 0) {
                throw new IllegalArgumentException("区画 " + section.name + " の始まりか長さが合わない");
            }
            ByteBuffer column = ByteBuffer.wrap(blob, (int) start, (int) length).slice();
            columns.put(section.name, column.order(ByteOrder.LITTLE_ENDIAN));
        }
        return new Scene(major, minor, chunks, sections, Collections.unmodifiableMap(columns));
    }

    private static Constants.SceneSection section(String name) {
        for (Constants.SceneSection section : Constants.SCENE_SECTIONS) {
            if (section.name.equals(name)) {
                return section;
            }
        }
        throw new IllegalArgumentException("知らない区画: " + name);
    }

    private ByteBuffer column(String name, Constants.ColumnType type) {
        Constants.SceneSection section = section(name);
        if (section.type != type) {
            throw new IllegalArgumentException("区画 " + name + " の値の型は " + section.type);
        }
        return columns.get(name).duplicate().order(ByteOrder.LITTLE_ENDIAN);
    }

    /** 区画の列を、写さずに見る（リトルエンディアンの読み出し専用のバッファー）。行ごとに width 個の値を並べる */
    public ByteBuffer buffer(String name) {
        return column(name, section(name).type).asReadOnlyBuffer().order(ByteOrder.LITTLE_ENDIAN);
    }

    /** 区画の行の数 */
    public int rows(String name) {
        Constants.SceneSection section = section(name);
        return columns.get(name).remaining() / (section.type.size * section.width);
    }

    /** u8 の区画の値（写し） */
    public byte[] u8(String name) {
        ByteBuffer column = column(name, Constants.ColumnType.U8);
        byte[] out = new byte[column.remaining()];
        column.get(out);
        return out;
    }

    /** u32 の区画の値（写し）。2^31 以上の値は負の int になるので、{@code value & 0xffffffffL} で読む */
    public int[] u32(String name) {
        ByteBuffer column = column(name, Constants.ColumnType.U32);
        int[] out = new int[column.remaining() / 4];
        column.asIntBuffer().get(out);
        return out;
    }

    /** f32 の区画の値（写し） */
    public float[] f32(String name) {
        ByteBuffer column = column(name, Constants.ColumnType.F32);
        float[] out = new float[column.remaining() / 4];
        column.asFloatBuffer().get(out);
        return out;
    }

    /** f64 の区画の値（写し） */
    public double[] f64(String name) {
        ByteBuffer column = column(name, Constants.ColumnType.F64);
        double[] out = new double[column.remaining() / 8];
        column.asDoubleBuffer().get(out);
        return out;
    }

    /**
     * strings の区画から、始まりと長さ（バイト）で文字列を引く
     *
     * @throws IndexOutOfBoundsException strings の区画の外を指すとき
     * @throws IllegalArgumentException UTF-8 として読めないとき
     */
    public String text(long start, long length) {
        ByteBuffer strings = column("strings", Constants.ColumnType.U8);
        if (start < 0 || length < 0 || start + length > strings.remaining()) {
            throw new IndexOutOfBoundsException("strings の区画の外を指す");
        }
        byte[] out = new byte[(int) length];
        strings.position((int) start);
        strings.get(out);
        return Json.decode(out);
    }
}
