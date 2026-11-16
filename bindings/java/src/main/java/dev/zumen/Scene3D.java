package dev.zumen;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * 読んだシーンの blob（#417・#421）。配置は schema/scene3d.toml にあり、セクションの定義は {@link Scene3DLayout} に生成する。
 *
 * <p>ヘッダーは {@code ZMS3}、メジャーバージョン（u16）、マイナーバージョン（u16）、塊の数（u32）、セクションの数（u32）で、続けてセクションごとに始まりと長さ
 * （u32、バイト）を置く。数はリトルエンディアン。知らないセクションは読み飛ばす。基本の 20 セクションがないときは
 * 読まない。メジャーバージョンが違う場合も読まない。
 */
public final class Scene3D {
    private static final int HEADER = 16;
    private static final int ENTRY = 8;

    /** メジャーバージョン */
    public final int major;
    /** マイナーバージョン */
    public final int minor;
    /** 塊の数 */
    public final long chunks;
    /** blob にあるセクションの数。このラッパーが知らないセクションも数える */
    public final long sections;

    private final Map<String, ByteBuffer> columns;

    private Scene3D(int major, int minor, long chunks, long sections, Map<String, ByteBuffer> columns) {
        this.major = major;
        this.minor = minor;
        this.chunks = chunks;
        this.sections = sections;
        this.columns = columns;
    }

    /**
     * シーンの blob を読む。列は blob のバイト列を共有するので、blob を書き換えない
     *
     * @throws IllegalArgumentException シーンの blob でないか、メジャーバージョンが違うか、セクションが blob に収まらないとき
     */
    public static Scene3D read(byte[] blob) {
        if (blob.length > 536870912) throw new IllegalArgumentException("3D シーンのバイト数の上限");
        ByteBuffer view = ByteBuffer.wrap(blob).order(ByteOrder.LITTLE_ENDIAN);
        if (blob.length < HEADER
                || blob[0] != 'Z'
                || blob[1] != 'M'
                || blob[2] != 'S'
                || blob[3] != '3') {
            throw new IllegalArgumentException("シーンの blob でない");
        }
        int major = view.getShort(4) & 0xffff;
        int minor = view.getShort(6) & 0xffff;
        long chunks = view.getInt(8) & 0xffffffffL;
        long sections = view.getInt(12) & 0xffffffffL;
        if (major != Scene3DLayout.MAJOR) {
            throw new IllegalArgumentException(
                    "シーンのメジャーバージョンが " + major + "（このラッパーは " + Scene3DLayout.MAJOR + "）");
        }
        if (HEADER + ENTRY * sections > blob.length) {
            throw new IllegalArgumentException("セクションの表が blob に収まらない");
        }
        if (sections < Scene3DLayout.SECTIONS.length) throw new IllegalArgumentException("3D シーンの基本列がない");
        long previous = HEADER + ENTRY * sections;
        for (int i = 0; i < sections; i++) {
            long start = view.getInt(HEADER + ENTRY*i) & 0xffffffffL;
            long length = view.getInt(HEADER + ENTRY*i + 4) & 0xffffffffL;
            if (start % 8 != 0 || start < previous || start + length > blob.length)
                throw new IllegalArgumentException("3D シーンのセクションの範囲");
            previous = start + length;
        }
        Map<String, ByteBuffer> columns = new LinkedHashMap<String, ByteBuffer>();
        for (int index = 0; index < Scene3DLayout.SECTIONS.length; index++) {
            Constants.SceneSection section = Scene3DLayout.SECTIONS[index];
            if (index >= sections) {
                columns.put(section.name, ByteBuffer.allocate(0).order(ByteOrder.LITTLE_ENDIAN));
                continue;
            }
            long start = view.getInt(HEADER + ENTRY * index) & 0xffffffffL;
            long length = view.getInt(HEADER + ENTRY * index + 4) & 0xffffffffL;
            long row = (long) section.type.size * section.width;
            if (start + length > blob.length || length % row != 0) {
                throw new IllegalArgumentException("セクション " + section.name + " の始まりか長さが合わない");
            }
            ByteBuffer column = ByteBuffer.wrap(blob, (int) start, (int) length).slice();
            columns.put(section.name, column.order(ByteOrder.LITTLE_ENDIAN));
        }
        return new Scene3D(major, minor, chunks, sections, Collections.unmodifiableMap(columns));
    }

    private long reference(String name, int row, int component) {
        int width = section(name).width;
        if (row < 0 || row >= rows(name) || component < 0 || component >= width)
            throw new IndexOutOfBoundsException("3D シーンの行の範囲");
        return columns.get(name).getInt((row*width+component)*4) & 0xffffffffL;
    }
    private long instanceReference(String name, String target, int row) {
        long value = reference(name, row, 0);
        if (value >= rows(target)) throw new IllegalArgumentException("3D シーンの参照");
        return value;
    }
    /** 元の建物の要素 ID */
    public String elementId(int row) { return text(reference("element_ids", row, 0), reference("element_ids", row, 1)); }
    /** インスタンスが指すメッシュの行 */
    public long instanceMesh(int row) { return instanceReference("instance_meshes", "mesh_vertices", row); }
    /** インスタンスが指す要素の行 */
    public long instanceElement(int row) { return instanceReference("instance_elements", "element_ids", row); }
    /** インスタンスが指す色の行 */
    public long instanceColor(int row) { return instanceReference("instance_colors", "color_rgba", row); }

    private static Constants.SceneSection section(String name) {
        for (Constants.SceneSection section : Scene3DLayout.SECTIONS) {
            if (section.name.equals(name)) {
                return section;
            }
        }
        throw new IllegalArgumentException("知らないセクション: " + name);
    }

    private ByteBuffer column(String name, Constants.ColumnType type) {
        Constants.SceneSection section = section(name);
        if (section.type != type) {
            throw new IllegalArgumentException("セクション " + name + " の値の型は " + section.type);
        }
        return columns.get(name).duplicate().order(ByteOrder.LITTLE_ENDIAN);
    }

    /** セクションの列を、写さずに見る（リトルエンディアンの読み出し専用のバッファー）。行ごとに width 個の値を並べる */
    public ByteBuffer buffer(String name) {
        return column(name, section(name).type).asReadOnlyBuffer().order(ByteOrder.LITTLE_ENDIAN);
    }

    /** セクションの行の数 */
    public int rows(String name) {
        Constants.SceneSection section = section(name);
        return columns.get(name).remaining() / (section.type.size * section.width);
    }

    /** u8 のセクションの値（写し） */
    public byte[] u8(String name) {
        ByteBuffer column = column(name, Constants.ColumnType.U8);
        byte[] out = new byte[column.remaining()];
        column.get(out);
        return out;
    }

    /** u32 のセクションの値（写し）。2^31 以上の値は負の int になるので、{@code value & 0xffffffffL} で読む */
    public int[] u32(String name) {
        ByteBuffer column = column(name, Constants.ColumnType.U32);
        int[] out = new int[column.remaining() / 4];
        column.asIntBuffer().get(out);
        return out;
    }

    /** f32 のセクションの値（写し） */
    public float[] f32(String name) {
        ByteBuffer column = column(name, Constants.ColumnType.F32);
        float[] out = new float[column.remaining() / 4];
        column.asFloatBuffer().get(out);
        return out;
    }

    /** f64 のセクションの値（写し） */
    public double[] f64(String name) {
        ByteBuffer column = column(name, Constants.ColumnType.F64);
        double[] out = new double[column.remaining() / 8];
        column.asDoubleBuffer().get(out);
        return out;
    }

    /**
     * strings のセクションから、始まりと長さ（バイト）で文字列を引く
     *
     * @throws IndexOutOfBoundsException strings のセクションの外を指すとき
     * @throws IllegalArgumentException UTF-8 として読めないとき
     */
    public String text(long start, long length) {
        ByteBuffer strings = column("strings", Constants.ColumnType.U8);
        if (start < 0 || length < 0 || start > strings.remaining() || length > strings.remaining() - start) {
            throw new IndexOutOfBoundsException("strings のセクションの外を指す");
        }
        byte[] out = new byte[(int) length];
        strings.position((int) start);
        strings.get(out);
        return Json.decode(out);
    }
}
