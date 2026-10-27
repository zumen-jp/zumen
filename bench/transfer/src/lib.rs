//! バイナリの受け渡しの形式を比べる試作（#6）。
//!
//! 合成した折れ線のシーンを、JSON・独自の列指向・FlatBuffers・Arrow IPC で符号化する。
//! シーンは塊ごとに f64 の原点と、そこからの f32 の相対座標を持つ（決定 0019）。

/// 塊ごとの原点と、折れ線の属性と頂点を列で持つシーン。
pub struct Scene {
    /// 塊の原点（x, y の順、m）。
    pub origins: Vec<f64>,
    /// 塊ごとの最初の折れ線の番号。最後に折れ線の数を足す。
    pub chunk_start: Vec<u32>,
    /// 折れ線ごとの最初の頂点の番号。最後に頂点の数を足す。
    pub vertex_start: Vec<u32>,
    /// 折れ線の画層の番号。
    pub layer: Vec<u32>,
    /// 折れ線の色（RGBA）。
    pub color: Vec<u32>,
    /// 折れ線の幅（mm）。
    pub width: Vec<f32>,
    /// 塊の原点からの相対座標（x, y の順、m）。
    pub coords: Vec<f32>,
}

impl Scene {
    /// 塊の数。
    pub fn chunks(&self) -> usize {
        self.origins.len() / 2
    }

    /// 折れ線の数。
    pub fn polylines(&self) -> usize {
        self.layer.len()
    }

    /// 頂点の数。
    pub fn vertices(&self) -> usize {
        self.coords.len() / 2
    }
}

/// 1 つの塊に入れる折れ線の数。
pub const CHUNK: usize = 1024;

/// 合成のシーンを作る。同じ引数からは同じシーンを作る。
///
/// 平面直角座標の 10⁵ m 級の範囲（1 km 四方）に、2〜8 点の折れ線を置く。
/// 塊は 32 m 四方の区画とし、原点はその角にする。
pub fn synthetic(polylines: usize, seed: u64) -> Scene {
    let mut state = seed;
    let mut next = move || {
        // 64 ビットの線形合同法（Knuth の MMIX の係数）。
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as u32
    };
    let mut scene = Scene {
        origins: Vec::new(),
        chunk_start: Vec::new(),
        vertex_start: Vec::new(),
        layer: Vec::new(),
        color: Vec::new(),
        width: Vec::new(),
        coords: Vec::new(),
    };
    for index in 0..polylines {
        if index % CHUNK == 0 {
            let cell = (index / CHUNK) as u32;
            scene.origins.push(100_000.0 + f64::from(cell % 31) * 32.0);
            scene.origins.push(50_000.0 + f64::from(cell / 31) * 32.0);
            scene.chunk_start.push(index as u32);
        }
        scene.vertex_start.push((scene.coords.len() / 2) as u32);
        scene.layer.push(next() % 64);
        scene.color.push(next() | 0xff);
        scene
            .width
            .push([0.13, 0.18, 0.25, 0.35, 0.5][(next() % 5) as usize]);
        let count = 2 + next() % 7;
        let mut x = (next() % 32_000) as f32 / 1000.0;
        let mut y = (next() % 32_000) as f32 / 1000.0;
        for _ in 0..count {
            scene.coords.push(x);
            scene.coords.push(y);
            x = (x + (next() % 2_000) as f32 / 1000.0 - 1.0).clamp(0.0, 32.0);
            y = (y + (next() % 2_000) as f32 / 1000.0 - 1.0).clamp(0.0, 32.0);
        }
    }
    scene.chunk_start.push(polylines as u32);
    scene.vertex_start.push((scene.coords.len() / 2) as u32);
    scene
}

/// 旧版の決定 0016 に近い、図形ごとのオブジェクトを並べる JSON。
#[cfg(feature = "json")]
pub fn json(scene: &Scene) -> Vec<u8> {
    use std::fmt::Write;
    let mut out = String::with_capacity(scene.coords.len() * 8);
    out.push_str("{\"version\":1,\"chunks\":[");
    for chunk in 0..scene.chunks() {
        if chunk > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"origin\":[{},{}],\"polylines\":[",
            scene.origins[chunk * 2],
            scene.origins[chunk * 2 + 1]
        );
        let (first, last) = (scene.chunk_start[chunk], scene.chunk_start[chunk + 1]);
        for polyline in first as usize..last as usize {
            if polyline > first as usize {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"layer\":{},\"color\":{},\"width\":{},\"points\":[",
                scene.layer[polyline], scene.color[polyline], scene.width[polyline]
            );
            let (from, to) = (
                scene.vertex_start[polyline] as usize * 2,
                scene.vertex_start[polyline + 1] as usize * 2,
            );
            for (i, value) in scene.coords[from..to].iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let _ = write!(out, "{value}");
            }
            out.push_str("]}");
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out.into_bytes()
}

/// 独自の列指向の配置の見出しの大きさ（バイト）。
#[cfg(feature = "columnar")]
pub const HEADER: usize = 24;

/// 独自の列指向の配置の区画の数。
#[cfg(feature = "columnar")]
pub const SECTIONS: usize = 7;

/// 独自の列指向の配置。1 つのバッファーに列を 8 バイトの境界で並べる。
///
/// 見出しは `ZMSC`、大版（u16）、小版（u16）、塊・折れ線・頂点の数（u32）、区画の数（u32）。
/// 続けて区画ごとに始まりと長さ（u32）を置き、知らない区画は読み飛ばせるようにする。
/// 区画は、原点（f64）、塊の始まり（u32）、頂点の始まり（u32）、画層（u32）、
/// 色（u32）、幅（f32）、相対座標（f32）の順。数はすべてリトルエンディアン。
#[cfg(feature = "columnar")]
pub fn columnar(scene: &Scene) -> Vec<u8> {
    let sections: [Vec<u8>; SECTIONS] = [
        scene.origins.iter().flat_map(|v| v.to_le_bytes()).collect(),
        scene
            .chunk_start
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect(),
        scene
            .vertex_start
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect(),
        scene.layer.iter().flat_map(|v| v.to_le_bytes()).collect(),
        scene.color.iter().flat_map(|v| v.to_le_bytes()).collect(),
        scene.width.iter().flat_map(|v| v.to_le_bytes()).collect(),
        scene.coords.iter().flat_map(|v| v.to_le_bytes()).collect(),
    ];
    let mut out = Vec::new();
    out.extend_from_slice(b"ZMSC");
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    for count in [
        scene.chunks(),
        scene.polylines(),
        scene.vertices(),
        SECTIONS,
    ] {
        out.extend_from_slice(&(count as u32).to_le_bytes());
    }
    let table = out.len();
    out.resize(table + SECTIONS * 8, 0);
    for (index, bytes) in sections.iter().enumerate() {
        out.resize(out.len().next_multiple_of(8), 0);
        let start = out.len() as u32;
        let at = table + index * 8;
        out[at..at + 4].copy_from_slice(&start.to_le_bytes());
        out[at + 4..at + 8].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(bytes);
    }
    out
}

/// FlatBuffers の表 1 つに、独自の配置と同じ列をベクターで持つ。
///
/// スキーマは `table Scene { origins:[double]; chunk_start:[uint]; vertex_start:[uint];
/// layer:[uint]; color:[uint]; width:[float]; coords:[float]; }` にあたる。
#[cfg(feature = "flatbuffers")]
pub fn flatbuffers(scene: &Scene) -> Vec<u8> {
    let mut builder =
        ::flatbuffers::FlatBufferBuilder::with_capacity(scene.coords.len() * 4 + 1024);
    let fields = [
        builder.create_vector(&scene.origins).value(),
        builder.create_vector(&scene.chunk_start).value(),
        builder.create_vector(&scene.vertex_start).value(),
        builder.create_vector(&scene.layer).value(),
        builder.create_vector(&scene.color).value(),
        builder.create_vector(&scene.width).value(),
        builder.create_vector(&scene.coords).value(),
    ];
    let table = builder.start_table();
    for (index, offset) in fields.iter().enumerate() {
        builder.push_slot_always::<::flatbuffers::WIPOffset<::flatbuffers::Vector<u8>>>(
            4 + 2 * index as u16,
            ::flatbuffers::WIPOffset::new(*offset),
        );
    }
    let root = builder.end_table(table);
    builder.finish_minimal(root);
    builder.finished_data().to_vec()
}

/// Arrow IPC のストリームを 2 つ並べる。折れ線の表と、塊の原点の表。
///
/// 先頭の 8 バイトに 1 つめのストリームの長さ（u32 と詰め物）を置く。
/// 折れ線の表は `chunk`・`layer`・`color`（u32）、`width`（f32）、`points`（f32 のリスト、x, y の順）。
#[cfg(feature = "arrow")]
pub fn arrow(scene: &Scene) -> Vec<u8> {
    use arrow_array::{ArrayRef, Float32Array, Float64Array, ListArray, RecordBatch, UInt32Array};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    let mut chunk = Vec::with_capacity(scene.polylines());
    for index in 0..scene.chunks() {
        let count = scene.chunk_start[index + 1] - scene.chunk_start[index];
        chunk.extend(std::iter::repeat_n(index as u32, count as usize));
    }
    let offsets: Vec<i32> = scene.vertex_start.iter().map(|v| (*v * 2) as i32).collect();
    let item = Arc::new(Field::new("item", DataType::Float32, false));
    let points = ListArray::new(
        item.clone(),
        arrow_buffer::OffsetBuffer::new(arrow_buffer::ScalarBuffer::from(offsets)),
        Arc::new(Float32Array::from(scene.coords.clone())),
        None,
    );
    let polylines = Arc::new(Schema::new(vec![
        Field::new("chunk", DataType::UInt32, false),
        Field::new("layer", DataType::UInt32, false),
        Field::new("color", DataType::UInt32, false),
        Field::new("width", DataType::Float32, false),
        Field::new("points", DataType::List(item), false),
    ]));
    let columns: Vec<ArrayRef> = vec![
        Arc::new(UInt32Array::from(chunk)),
        Arc::new(UInt32Array::from(scene.layer.clone())),
        Arc::new(UInt32Array::from(scene.color.clone())),
        Arc::new(Float32Array::from(scene.width.clone())),
        Arc::new(points),
    ];
    let first = stream(RecordBatch::try_new(polylines, columns).ok());
    let (xs, ys): (Vec<f64>, Vec<f64>) = scene.origins.chunks(2).map(|p| (p[0], p[1])).unzip();
    let origins = Arc::new(Schema::new(vec![
        Field::new("x", DataType::Float64, false),
        Field::new("y", DataType::Float64, false),
    ]));
    let columns: Vec<ArrayRef> = vec![
        Arc::new(Float64Array::from(xs)),
        Arc::new(Float64Array::from(ys)),
    ];
    let second = stream(RecordBatch::try_new(origins, columns).ok());
    let mut out = Vec::with_capacity(8 + first.len() + second.len());
    out.extend_from_slice(&(first.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&first);
    out.extend_from_slice(&second);
    out
}

#[cfg(feature = "arrow")]
fn stream(batch: Option<arrow_array::RecordBatch>) -> Vec<u8> {
    let mut out = Vec::new();
    if let Some(batch) = batch
        && let Ok(mut writer) = arrow_ipc::writer::StreamWriter::try_new(&mut out, &batch.schema())
    {
        let _ = writer.write(&batch);
        let _ = writer.finish();
    }
    out
}

/// wasm の大きさを比べるための入り口。有効にした形式だけを符号化して、長さを返す。
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
pub extern "C" fn bench_encode(polylines: u32) -> u32 {
    let scene = synthetic(polylines as usize, 1);
    #[allow(unused_mut)]
    let mut length = scene.vertices();
    #[cfg(feature = "json")]
    {
        length += json(&scene).len();
    }
    #[cfg(feature = "columnar")]
    {
        length += columnar(&scene).len();
    }
    #[cfg(feature = "flatbuffers")]
    {
        length += flatbuffers(&scene).len();
    }
    #[cfg(feature = "arrow")]
    {
        length += arrow(&scene).len();
    }
    length as u32
}
