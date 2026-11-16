//! バッファーを借用して列を読む。確保と全体の検査は read で予算を数える。
use crate::{Error, Instance, MAX_BYTES, layout as l};
use std::collections::BTreeSet;
use zumen_base::{Cx, Kind};

/// 型と幅を持つ列。バイト列は元の blob を借用する。
#[derive(Clone, Copy, Debug)]
pub struct Column<'a> {
    /// 列の定義
    pub definition: l::Section,
    data: &'a [u8],
}

impl<'a> Column<'a> {
    /// 行の数
    pub fn rows(self) -> u64 {
        (self.data.len() / self.definition.ty.size() / self.definition.width) as u64
    }

    /// 元のリトルエンディアンのバイト列
    pub fn bytes(self) -> &'a [u8] {
        self.data
    }

    fn value<const N: usize>(self, index: u64) -> Option<[u8; N]> {
        let start = usize::try_from(index.checked_mul(N as u64)?).ok()?;
        self.data.get(start..start.checked_add(N)?)?.try_into().ok()
    }

    /// u32 の値。行内の値も続けて数える
    pub fn u32(self, index: u64) -> Option<u32> {
        (self.definition.ty == l::Type::U32).then(|| self.value(index).map(u32::from_le_bytes))?
    }

    /// f32 の値。行内の値も続けて数える
    pub fn f32(self, index: u64) -> Option<f32> {
        (self.definition.ty == l::Type::F32).then(|| self.value(index).map(f32::from_le_bytes))?
    }

    /// f64 の値。行内の値も続けて数える
    pub fn f64(self, index: u64) -> Option<f64> {
        (self.definition.ty == l::Type::F64).then(|| self.value(index).map(f64::from_le_bytes))?
    }

    fn slice(self, begin: u64, end: u64) -> Result<Self, Error> {
        let width = (self.definition.width * self.definition.ty.size()) as u64;
        let start = usize::try_from(begin.checked_mul(width).ok_or(Error::Invalid("range"))?)
            .map_err(|_| Error::Invalid("range"))?;
        let end = usize::try_from(end.checked_mul(width).ok_or(Error::Invalid("range"))?)
            .map_err(|_| Error::Invalid("range"))?;
        Ok(Self {
            data: self.data.get(start..end).ok_or(Error::Invalid("range"))?,
            ..self
        })
    }
}

/// 1 つの共有メッシュの列。
#[derive(Clone, Copy, Debug)]
pub struct MeshView<'a> {
    /// 局所座標
    pub vertices: Column<'a>,
    /// このメッシュの中の頂点番号
    pub triangles: Column<'a>,
    /// このメッシュの中の頂点番号
    pub edges: Column<'a>,
}

/// 要素の文字列と階への参照。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElementView<'a> {
    /// 建物モデルの ID
    pub id: &'a str,
    /// クラス
    pub class: &'a str,
    /// 名前。なければ空
    pub name: &'a str,
    /// 階の要素番号
    pub storey: Option<u32>,
}

/// 検査済みのシーン。未知のセクションは保持せず読み飛ばす。
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    /// メジャーバージョン
    pub major: u16,
    /// マイナーバージョン
    pub minor: u16,
    /// 塊の数
    pub chunks: u32,
    /// blob のセクション数。未知の列も含む
    pub sections: u32,
    columns: [Column<'a>; l::SECTIONS.len()],
}

impl<'a> Reader<'a> {
    /// 生成したセクション番号から列を引く
    pub fn column(&self, index: usize) -> Option<Column<'a>> {
        self.columns.get(index).copied()
    }

    fn col(&self, index: usize) -> Result<Column<'a>, Error> {
        self.column(index).ok_or(Error::Invalid("column"))
    }

    /// strings の始まりと長さから、UTF-8 の文字列を引く
    pub fn text(&self, start: u32, length: u32) -> Result<&'a str, Error> {
        let data = self.col(l::STRINGS)?.data;
        let end = usize::try_from(u64::from(start) + u64::from(length))
            .map_err(|_| Error::Invalid("string"))?;
        std::str::from_utf8(
            data.get(start as usize..end)
                .ok_or(Error::Invalid("string"))?,
        )
        .map_err(|_| Error::Invalid("utf8"))
    }

    /// ID・クラス・名前の列から行を引く
    pub fn string(&self, section: usize, row: u64) -> Result<&'a str, Error> {
        let column = self.col(section)?;
        if column.definition.ty != l::Type::U32
            || column.definition.width != 2
            || row >= column.rows()
        {
            return Err(Error::Invalid("string"));
        }
        self.text(
            required(column.u32(row * 2))?,
            required(column.u32(row * 2 + 1))?,
        )
    }

    /// 1 つの要素を引く
    pub fn element(&self, row: u64) -> Result<ElementView<'a>, Error> {
        let storey = required(self.col(l::ELEMENT_STOREYS)?.u32(row))?;
        Ok(ElementView {
            id: self.string(l::ELEMENT_IDS, row)?,
            class: self.string(l::ELEMENT_CLASSES, row)?,
            name: self.string(l::ELEMENT_NAMES, row)?,
            storey: (storey != l::NONE).then_some(storey),
        })
    }

    /// 1 つの配置から、メッシュ・要素・色と変換を引く
    pub fn instance(&self, row: u64) -> Result<Instance, Error> {
        let column = self.col(l::INSTANCE_TRANSFORMS)?;
        if row >= column.rows() {
            return Err(Error::Invalid("instance"));
        }
        let mut transform = [0.0; 12];
        for (index, value) in transform.iter_mut().enumerate() {
            *value = required(column.f32(row * 12 + index as u64))?;
        }
        Ok(Instance {
            mesh: required(self.col(l::INSTANCE_MESHES)?.u32(row))?,
            element: required(self.col(l::INSTANCE_ELEMENTS)?.u32(row))?,
            color: required(self.col(l::INSTANCE_COLORS)?.u32(row))?,
            transform,
        })
    }

    fn mesh_column(&self, row: u64, starts: usize, target: usize) -> Result<Column<'a>, Error> {
        let starts = self.col(starts)?;
        if row >= starts.rows() {
            return Err(Error::Invalid("mesh"));
        }
        let target = self.col(target)?;
        let begin = u64::from(required(starts.u32(row))?);
        let end = starts.u32(row + 1).map(u64::from).unwrap_or(target.rows());
        target.slice(begin, end)
    }

    /// メッシュの頂点・三角形・稜線を、コピーせずに引く
    pub fn mesh(&self, row: u64) -> Result<MeshView<'a>, Error> {
        Ok(MeshView {
            vertices: self.mesh_column(row, l::MESH_VERTICES, l::VERTEX_XYZ)?,
            triangles: self.mesh_column(row, l::MESH_TRIANGLES, l::TRIANGLE_VERTICES)?,
            edges: self.mesh_column(row, l::MESH_EDGES, l::EDGE_VERTICES)?,
        })
    }
}

fn required<T>(value: Option<T>) -> Result<T, Error> {
    value.ok_or(Error::Invalid("column"))
}

fn number<const N: usize>(bytes: &[u8], start: usize) -> Result<[u8; N], Error> {
    bytes
        .get(start..start.checked_add(N).ok_or(Error::Invalid("size"))?)
        .and_then(|s| s.try_into().ok())
        .ok_or(Error::Invalid("header"))
}

/// ヘッダー・列・有限値・番号の参照・塊の所属を検査して読む。元のバイト列はコピーしない。
pub fn read<'a>(bytes: &'a [u8], cx: &mut Cx) -> Result<Reader<'a>, Error> {
    cx.check(Kind::InputBytes, bytes.len() as u64)?;
    read_inner(bytes, cx)
}

pub(crate) fn read_inner<'a>(bytes: &'a [u8], cx: &mut Cx) -> Result<Reader<'a>, Error> {
    cx.check(Kind::Work, 1)?;
    if bytes.len() as u64 > MAX_BYTES || number::<4>(bytes, 0)? != l::MAGIC {
        return Err(Error::Invalid("header"));
    }
    let major = u16::from_le_bytes(number(bytes, 4)?);
    if major != l::MAJOR {
        return Err(Error::Invalid("version"));
    }
    let minor = u16::from_le_bytes(number(bytes, 6)?);
    let chunks = u32::from_le_bytes(number(bytes, 8)?);
    let sections = u32::from_le_bytes(number(bytes, 12)?);
    let table_end = 16 + u64::from(sections) * 8;
    if table_end > bytes.len() as u64 || (sections as usize) < l::SECTIONS.len() {
        return Err(Error::Invalid("sections"));
    }
    cx.check(Kind::Work, u64::from(sections))?;
    let mut reader = Reader {
        major,
        minor,
        chunks,
        sections,
        columns: l::SECTIONS.map(|definition| Column {
            definition,
            data: &[],
        }),
    };
    let mut end = table_end;
    for index in 0..sections as usize {
        cx.check(Kind::Work, 0)?;
        let start = u64::from(u32::from_le_bytes(number(bytes, 16 + index * 8)?));
        let length = u64::from(u32::from_le_bytes(number(bytes, 20 + index * 8)?));
        if start % 8 != 0 || start < end || start + length > bytes.len() as u64 {
            return Err(Error::Invalid("section_range"));
        }
        end = start + length;
        if let Some(column) = reader.columns.get_mut(index) {
            let row_bytes = column.definition.ty.size() * column.definition.width;
            if length % row_bytes as u64 != 0 {
                return Err(Error::Invalid("row"));
            }
            column.data = bytes
                .get(start as usize..end as usize)
                .ok_or(Error::Invalid("section_range"))?;
        }
    }
    reader.validate(cx)?;
    Ok(reader)
}

fn starts(column: Column<'_>, target_rows: u64, cx: &mut Cx) -> Result<(), Error> {
    cx.check(Kind::Work, column.rows())?;
    if (column.rows() == 0 && target_rows != 0) || (column.rows() != 0 && column.u32(0) != Some(0))
    {
        return Err(Error::Invalid("starts"));
    }
    let mut last = 0;
    for index in 0..column.rows() {
        let value = u64::from(required(column.u32(index))?);
        if value < last || value > target_rows {
            return Err(Error::Invalid("starts"));
        }
        last = value;
    }
    Ok(())
}

impl Reader<'_> {
    fn same_rows(&self, sections: &[usize], rows: u64) -> Result<(), Error> {
        for &section in sections {
            if self.col(section)?.rows() != rows {
                return Err(Error::Invalid("row_count"));
            }
        }
        Ok(())
    }

    fn validate(&self, cx: &mut Cx) -> Result<(), Error> {
        let meshes = self.col(l::MESH_VERTICES)?.rows();
        let instances = self.col(l::INSTANCE_MESHES)?.rows();
        let elements = self.col(l::ELEMENT_IDS)?.rows();
        let colors = self.col(l::COLOR_RGBA)?.rows();
        self.same_rows(
            &[l::CHUNK_ORIGINS, l::CHUNK_BOUNDS, l::CHUNK_INSTANCES],
            u64::from(self.chunks),
        )?;
        self.same_rows(&[l::MESH_TRIANGLES, l::MESH_EDGES], meshes)?;
        self.same_rows(
            &[
                l::INSTANCE_ELEMENTS,
                l::INSTANCE_COLORS,
                l::INSTANCE_TRANSFORMS,
            ],
            instances,
        )?;
        self.same_rows(
            &[l::ELEMENT_CLASSES, l::ELEMENT_NAMES, l::ELEMENT_STOREYS],
            elements,
        )?;
        self.same_rows(&[l::COLOR_FLAGS], colors)?;
        for column in &self.columns {
            cx.check(
                Kind::Work,
                column.data.len() as u64 / column.definition.ty.size() as u64,
            )?;
            let invalid = match column.definition.ty {
                l::Type::F32 => column
                    .data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|v| !f32::from_le_bytes(*v).is_finite()),
                l::Type::F64 => column
                    .data
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .any(|v| !f64::from_le_bytes(*v).is_finite()),
                _ => false,
            };
            if invalid {
                return Err(Error::Invalid("finite"));
            }
        }
        std::str::from_utf8(self.col(l::STRINGS)?.data).map_err(|_| Error::Invalid("utf8"))?;
        for (source, target) in [
            (l::MESH_VERTICES, l::VERTEX_XYZ),
            (l::MESH_TRIANGLES, l::TRIANGLE_VERTICES),
            (l::MESH_EDGES, l::EDGE_VERTICES),
            (l::CHUNK_INSTANCES, l::INSTANCE_MESHES),
        ] {
            starts(self.col(source)?, self.col(target)?.rows(), cx)?;
        }
        for row in 0..meshes {
            cx.check(Kind::Work, 1)?;
            let mesh = self.mesh(row)?;
            for column in [mesh.triangles, mesh.edges] {
                cx.check(Kind::Work, column.rows() * column.definition.width as u64)?;
                for bytes in column.data.as_chunks::<4>().0 {
                    if u64::from(u32::from_le_bytes(*bytes)) >= mesh.vertices.rows() {
                        return Err(Error::Invalid("vertex_reference"));
                    }
                }
            }
        }
        let mut ids = BTreeSet::new();
        for row in 0..elements {
            let element = self.element(row)?;
            cx.check(
                Kind::Work,
                (element.id.len() as u64 + 1) * (row + 1).ilog2() as u64 + 1,
            )?;
            cx.check(Kind::DecodedBytes, 256)?;
            if element.id.is_empty() || !ids.insert(element.id) {
                return Err(Error::Invalid("element_id"));
            }
            if element
                .storey
                .is_some_and(|value| u64::from(value) >= elements)
            {
                return Err(Error::Invalid("storey"));
            }
        }
        for bytes in self.col(l::COLOR_RGBA)?.data.as_chunks::<4>().0 {
            let value = f32::from_le_bytes(*bytes);
            if !(0.0..=1.0).contains(&value) {
                return Err(Error::Invalid("color"));
            }
        }
        for (row, &flag) in self.col(l::COLOR_FLAGS)?.data.iter().enumerate() {
            if flag & l::color_flag::UNRESOLVED != 0 {
                for axis in 0..3 {
                    if self.col(l::COLOR_RGBA)?.f32(row as u64 * 4 + axis) != Some(0.0) {
                        return Err(Error::Invalid("unresolved_color"));
                    }
                }
            }
        }
        cx.check(Kind::DecodedBytes, elements * 4)?;
        let count = usize::try_from(elements).map_err(|_| Error::Invalid("size"))?;
        let mut owners = vec![l::NONE; count];
        for chunk in 0..u64::from(self.chunks) {
            cx.check(Kind::Work, 1)?;
            let bounds = self.col(l::CHUNK_BOUNDS)?;
            for axis in 0..3 {
                if required(bounds.f32(chunk * 6 + axis))?
                    > required(bounds.f32(chunk * 6 + axis + 3))?
                {
                    return Err(Error::Invalid("bounds"));
                }
            }
            let begins = self.col(l::CHUNK_INSTANCES)?;
            let begin = u64::from(required(begins.u32(chunk))?);
            let end = begins.u32(chunk + 1).map(u64::from).unwrap_or(instances);
            for row in begin..end {
                cx.check(Kind::Work, 1)?;
                let instance = self.instance(row)?;
                if u64::from(instance.mesh) >= meshes || u64::from(instance.color) >= colors {
                    return Err(Error::Invalid("instance_reference"));
                }
                let owner = owners
                    .get_mut(instance.element as usize)
                    .ok_or(Error::Invalid("element_reference"))?;
                if *owner != l::NONE && u64::from(*owner) != chunk {
                    return Err(Error::Invalid("element_chunk"));
                }
                *owner = chunk as u32;
                let [a, b, c, _, d, e, f, _, g, h, i, _] = instance.transform.map(f64::from);
                if a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g) == 0.0 {
                    return Err(Error::Invalid("transform"));
                }
            }
        }
        cx.check(Kind::Work, 0)?;
        Ok(())
    }
}
