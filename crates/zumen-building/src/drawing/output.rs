//! 切り出した線と図面モデル、元の建物への対応。

use zumen_base::{Cx, Key, Kind, Ref};
use zumen_drawing::{self as d, Drawing2D, ModelPoint, SpaceId};
use zumen_geom2d::topology::Grid;
use zumen_geom3d::{Failure, Xy};

use super::{
    Frame,
    budget::{push, reserve, work},
};
use crate::{ElementRef, GeometryRef, Id};

/// 元の建物の要素・形の項目。図面の行番号を永続 ID にしない。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// 元の建物の要素の参照。
    pub element: ElementRef,
    /// 元の建物の要素の ID。
    pub id: Id,
    /// ファイルにあった GlobalId。ID に使えなかった値も保つ。
    pub global_id: Option<String>,
    /// 元の要素の STEP インスタンス。
    pub instance: Option<u64>,
    /// 元の要素が所有する形の項目。形によらない注記なら None。
    pub item: Option<GeometryRef>,
    /// 形の項目の STEP インスタンス。
    pub shape_instance: Option<u64>,
}

impl Source {
    pub(super) fn place(&self) -> Ref {
        let place = Ref::default().with_id(self.id.to_string());
        match self.shape_instance.or(self.instance) {
            Some(i) => place.with_key(Key::Instance(i)),
            None => place,
        }
    }
}

/// 切り出した線の役割。線幅・線種などの慣習はプロファイルが決める。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// 面で切った線。
    Cut,
    /// 見えている形の辺。
    Visible,
    /// ほかの面の向こうに隠れた辺。
    Hidden,
    /// 通り芯。
    Grid,
    /// 注記、建具の平面の記号。
    Annotation,
}

impl Role {
    /// ID とレイヤに使う、版をまたいで保つ名前。
    pub fn name(self) -> &'static str {
        match self {
            Self::Cut => "cut",
            Self::Visible => "visible",
            Self::Hidden => "hidden",
            Self::Grid => "grid",
            Self::Annotation => "annotation",
        }
    }
    fn layer(self) -> d::LayerId {
        d::LayerId(self as u32)
    }
}

/// 図面の線から、派生 ID・役割・出所へ戻る対応。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// 建物の ID・役割・その役割の順で作る派生 ID。
    pub id: String,
    /// 図面モデルの ID。既存の図面の命名規則で作る。
    pub drawing_id: String,
    /// 実寸の空間の中の線の位置。
    pub index: usize,
    /// 線の役割。
    pub role: Role,
    /// Output.sources の位置。
    pub source: usize,
}

/// 図面の配置。縮尺は 1:denominator、紙上の値は mm。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper {
    /// 正の有限な縮尺の分母。
    pub denominator: f64,
    /// 図面原点を置く紙上の位置。
    pub origin: [d::PaperLength; 2],
    /// 用紙の幅と高さ。指定するなら両方正の有限値。
    pub size: Option<[d::PaperLength; 2]>,
}

/// 切り出しの処理量。時刻はコアの結果に含めない。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// 評価した要素の数。
    pub elements: u64,
    /// 投影した形の部品の数。
    pub parts: u64,
    /// 投影前の三角形の数。
    pub triangles: u64,
    /// 深さで切った三角形と切り口の領域の数。
    pub surfaces: u64,
    /// 可視性を調べる前の線の数。
    pub edges: u64,
    /// 出力した線の数。
    pub lines: u64,
    /// 3D の形の評価が完全でなかった要素の数。
    pub evaluation_failures: u64,
    /// 投影または切り口の位相の計算に失敗した部品の数。
    pub projection_failures: u64,
    /// 格子で点などへ縮んだ辺や輪郭の数。
    pub collapsed: u64,
}

/// 元の値を変更せずに作った図面と対応表。
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    /// 実寸の空間と用紙のビューを持つ図面。
    pub drawing: Drawing2D,
    /// 実寸の空間の各線と同じ順の、建物への対応。
    pub lines: Vec<Line>,
    /// 元の要素と形の項目。
    pub sources: Vec<Source>,
    /// プロジェクトと図面の座標変換。
    pub frame: Frame,
    /// 位相の計算に使った格子。原点・間隔は図面の文書単位。
    pub grid: Grid,
    /// 未対応の形や投影の失敗による欠落がなかったか。
    pub complete: bool,
    /// 決定的な処理量。
    pub statistics: Statistics,
}

#[derive(Clone, Debug)]
pub(super) struct Stroke {
    pub ends: [Xy; 2],
    pub source: usize,
    pub role: Role,
}

pub(super) fn validate_paper(paper: Paper, units: f64) -> Result<f64, Failure> {
    let scale = units * 1000.0 / paper.denominator;
    if !paper.denominator.is_finite()
        || paper.denominator <= 0.0
        || !scale.is_finite()
        || scale <= 0.0
        || paper.origin.iter().any(|v| !v.0.is_finite())
        || paper
            .size
            .is_some_and(|size| size.iter().any(|v| !v.0.is_finite() || v.0 <= 0.0))
    {
        return Err(Failure::Invalid("drawing_paper"));
    }
    Ok(scale)
}

pub(super) fn write(
    strokes: Vec<Stroke>,
    sources: Vec<Source>,
    frame: Frame,
    grid: Grid,
    unit: d::Unit,
    paper: Paper,
    cx: &mut Cx<'_>,
) -> Result<Output, Failure> {
    let scale = validate_paper(
        paper,
        unit.millimeters().ok_or(Failure::Invalid("drawing_unit"))? / 1000.0,
    )?;
    let space = SpaceId(0);
    let sheet = d::SheetId(0);
    // 表・用紙・ビューなどの固定部分。線の領域と文字列は別に数える。
    cx.check(Kind::DecodedBytes, 8192)?;
    let mut drawing = Drawing2D {
        spaces: vec![d::ModelSpace {
            id: space,
            unit,
            axes: d::Axes::EastNorth,
            elements: Vec::new(),
            order: Vec::new(),
        }],
        layers: [
            Role::Cut,
            Role::Visible,
            Role::Hidden,
            Role::Grid,
            Role::Annotation,
        ]
        .map(|role| d::Layer {
            id: role.layer(),
            name: role.name().into(),
            state: None,
            style: None,
            follows_placement: false,
            description: None,
            source: Ref::default(),
        })
        .into(),
        sheets: vec![d::Sheet {
            id: sheet,
            name: None,
            size: paper.size,
            elements: Vec::new(),
            views: vec![d::View {
                space,
                sheet,
                transform: d::Affine {
                    m: [scale, 0.0, paper.origin[0].0, 0.0, scale, paper.origin[1].0],
                },
                clip: Vec::new(),
                hidden_layers: Vec::new(),
                paper_unit: None,
                source: Ref::default(),
            }],
            order: vec![d::SheetItem::View(0)],
            source: Ref::default(),
        }],
        ..Drawing2D::default()
    };
    let mut lines = Vec::new();
    let mut counts = std::collections::BTreeMap::<(Id, Role), u64>::new();
    let model = drawing
        .spaces
        .first_mut()
        .ok_or(Failure::Invalid("drawing_space"))?;
    reserve(&mut model.elements, strokes.len(), 512, cx)?;
    reserve(&mut lines, strokes.len(), 96, cx)?;
    reserve(&mut model.order, strokes.len(), 8, cx)?;
    for stroke in strokes {
        work(cx, 64)?;
        let source = sources
            .get(stroke.source)
            .ok_or(Failure::Invalid("drawing_source"))?;
        if !counts.contains_key(&(source.id, stroke.role)) {
            cx.check(Kind::DecodedBytes, 128)?;
        }
        let ordinal = counts.entry((source.id, stroke.role)).or_default();
        cx.check(Kind::DecodedBytes, 256)?;
        let id = format!("building/{}/{}/{}", source.id, stroke.role.name(), ordinal);
        *ordinal += 1;
        let index = model.elements.len();
        // 出所の ID は派生 ID。描画用の既存の ID との対応を別に保ち、ほかの図面の ID を変えない。
        let reference = Ref::default().with_id(id.clone());
        let [start, end] = stroke.ends.map(|[x, y]| ModelPoint { space, x, y });
        model.elements.push(d::Element {
            source: reference,
            layer: stroke.role.layer(),
            style: d::Style {
                color: d::Inherit::Layer,
                line_type: d::Inherit::Layer,
                line_type_scale: 1.0,
                line_weight: None,
                transparency: d::Inherit::Layer,
            },
            shape: d::Shape::Line { start, end },
        });
        model.order.push(index);
        lines.push(Line {
            id,
            drawing_id: format!("space/0/e{index}"),
            index,
            role: stroke.role,
            source: stroke.source,
        });
    }
    // 一般の図面モデルだけを渡す利用側にも対応を残す。図形が編集されたらハッシュで失効する。
    for line in &lines {
        work(cx, 1)?;
        let source = sources
            .get(line.source)
            .ok_or(Failure::Invalid("drawing_source"))?;
        cx.check(Kind::DecodedBytes, 1024)?;
        let text = |key, value: String| d::Field {
            key: std::borrow::Cow::Borrowed(key),
            code: None,
            value: d::FieldValue::Text(value),
        };
        let mut fields = vec![
            text("id", line.id.clone()),
            text("element", source.id.to_string()),
            text("role", line.role.name().into()),
        ];
        reserve(&mut fields, 3, 80, cx)?;
        if let Some(value) = &source.global_id {
            cx.check(Kind::DecodedBytes, value.len() as u64)?;
            work(cx, value.len() as u64)?;
            fields.push(text("global_id", value.clone()));
        }
        for (key, value) in [
            ("instance", source.instance),
            ("shape_instance", source.shape_instance),
        ] {
            if let Some(value) = value {
                fields.push(d::Field {
                    key: std::borrow::Cow::Borrowed(key),
                    code: None,
                    value: d::FieldValue::Handle(value),
                });
            }
        }
        let extension = d::Extension {
            namespace: "zumen.building.drawing".into(),
            name: "line".into(),
            version: None,
            schema: 1,
            target: d::Target::Element(line.drawing_id.clone()),
            basis: d::Basis::Hash(d::Hash::element_checked(
                model
                    .elements
                    .get(line.index)
                    .ok_or(Failure::Invalid("drawing_index"))?,
                cx,
            )?),
            fields,
            source: source.place(),
        };
        push(&mut drawing.extensions, extension, 256, cx)?;
    }
    let statistics = Statistics {
        lines: lines.len() as u64,
        ..Statistics::default()
    };
    Ok(Output {
        drawing,
        lines,
        sources,
        frame,
        grid,
        complete: true,
        statistics,
    })
}
