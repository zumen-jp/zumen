//! 建物の切り出しに、版つきの線・模様・記号の慣習を当てる。
//! 建物の形、階の高さ、材料名、開閉形式を推測せず、元モデルを変更しない。

mod decorate;
mod joinery;
mod scale;
mod settings;

use crate::Identity;
use crate::building::{BuildingProfile, Error, reserve};
use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_building::{Building, ElementRef, drawing as b};
use zumen_crs::ProjectPoint;
use zumen_drawing as d;

pub use settings::Defaults;

/// 継承後の作図の慣習。JSON の表は読み込み時に検査して保持する。
#[derive(Clone, Debug)]
pub struct DrawingProfile {
    profile: BuildingProfile,
    settings: settings::Settings,
}

/// 元の線とは別に足した模様・通り芯の記号と、その出所。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decoration {
    /// 建物の ID、種類、その種類の順で作る派生 ID。
    pub id: String,
    /// 図面モデルの要素の ID。
    pub drawing_id: String,
    /// 実寸の空間の要素の位置。
    pub index: usize,
    /// 元の建物の要素。
    pub element: ElementRef,
    /// hatch・grid-circle・grid-label。
    pub kind: &'static str,
}

/// 作図の慣習を当てた図面。線の ID・順・元要素への対応は切り出しの結果を保つ。
#[derive(Clone, Debug)]
pub struct Drawing {
    /// 切り出した線と、追加の図形を含む図面。lines と statistics は元の線だけを数える。
    pub output: b::Output,
    /// 追加した図形の対応。元の線の対応と混ぜない。
    pub decorations: Vec<Decoration>,
    /// 適用したプロファイルの鎖。
    pub chain: Vec<Identity>,
    /// 継承後の値のハッシュ。
    pub hash: [u8; 32],
}

impl DrawingProfile {
    /// 既存の extends の置換規則で解き、全語彙の値を検査する。
    pub fn load(
        inputs: &[(&str, &[u8])],
        id: &str,
        edition: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        let profile = BuildingProfile::load(inputs, id, edition, cx)?;
        profile.plan_work(cx)?;
        let settings = settings::Settings::parse(&profile)?;
        cx.check(Kind::Work, 0)?;
        Ok(Self { profile, settings })
    }

    /// プロファイルに明記された既定値。高さと深さは m、紙上の値は mm。
    pub fn defaults(&self) -> Defaults {
        self.settings.defaults
    }

    /// 名前から level の種類を引く。高さや階番号は補わず、不明・複数一致は None を返す。
    pub fn level_kind<'a>(&'a self, name: &str, cx: &mut Cx<'_>) -> Result<Option<&'a str>, Error> {
        self.profile.plan_work(cx)?;
        cx.check(
            Kind::Work,
            (name.len() as u64).saturating_mul(self.settings.levels.len() as u64 + 1),
        )?;
        let mut found = None;
        for level in &self.settings.levels {
            let matches = level.names.iter().any(|v| v == name)
                || level.numbers.iter().any(|(prefix, suffix)| {
                    name.strip_prefix(prefix.as_str())
                        .and_then(|v| v.strip_suffix(suffix.as_str()))
                        .is_some_and(|number| !number.is_empty() && number.parse::<i32>().is_ok())
                });
            if matches && found.replace(level.kind.as_str()).is_some() {
                return Ok(None);
            }
        }
        Ok(found)
    }

    /// 呼び出し側が指定した階の基点に、プロファイルの高さと深さを文書単位へ換算して足す。
    pub fn default_plan(
        &self,
        building: &Building,
        floor: ProjectPoint,
        cx: &mut Cx<'_>,
    ) -> Result<b::View, Error> {
        let unit = length_unit(building, cx)?;
        let view = b::View::Plan {
            floor,
            height: self.settings.defaults.height / unit,
            depth: self.settings.defaults.depth / unit,
        };
        view.frame().map_err(Error::Geometry)?;
        Ok(view)
    }

    /// 呼び出し側が指定した切断線の始点・終点・見る側に、プロファイルの深さを文書単位へ換算して断面図の面を作る。
    pub fn default_section(
        &self,
        building: &Building,
        start: ProjectPoint,
        end: ProjectPoint,
        side: b::Side,
        cx: &mut Cx<'_>,
    ) -> Result<b::View, Error> {
        let unit = length_unit(building, cx)?;
        let view = b::View::Vertical {
            start,
            end,
            side,
            depth: self.settings.defaults.depth / unit,
            section: true,
        };
        view.frame().map_err(Error::Geometry)?;
        Ok(view)
    }

    /// 元の Plan の曲線、明記された開閉形式の記号、3D の切断の順で切り出し、慣習を当てる。
    pub fn cut(
        &self,
        building: &Building,
        view: b::View,
        tolerance: f64,
        chord: f64,
        paper: Option<b::Paper>,
        cx: &mut Cx<'_>,
    ) -> Result<Drawing, Error> {
        self.profile.plan_work(cx)?;
        let prepared = if matches!(view, b::View::Plan { .. }) {
            joinery::prepare(self, building, cx)?
        } else {
            Vec::new()
        };
        let mut symbols = Vec::new();
        reserve(&mut symbols, prepared.len(), 128, cx)?;
        for symbol in &prepared {
            symbols.push(symbol.rule());
        }
        let output = b::cut_with_attribute_symbols(
            building,
            b::Options {
                view,
                tolerance,
                chord,
                paper: paper.unwrap_or_else(|| self.settings.defaults.paper()),
                symbols: &[],
            },
            &symbols,
            cx,
        )
        .map_err(Error::Geometry)?;
        self.apply(building, output, cx)
    }

    /// 未加工の cut の結果を受け取り、線の表現と断面の模様・通り芯の記号を足す。
    /// 建具記号の選択も必要なら cut を使う。停止時には部分結果を返さない。
    pub fn apply(
        &self,
        building: &Building,
        mut output: b::Output,
        cx: &mut Cx<'_>,
    ) -> Result<Drawing, Error> {
        self.profile.plan_work(cx)?;
        let Some(model) = output.drawing.spaces.first() else {
            return Err(Error::Input("drawing_space"));
        };
        if output.drawing.spaces.len() != 1
            || model.id != d::SpaceId(0)
            || model.elements.len() != output.lines.len()
            || output.drawing.extensions.len() != output.lines.len()
        {
            return Err(Error::Input("drawing_modified"));
        }
        for (index, (line, extension)) in output
            .lines
            .iter()
            .zip(&output.drawing.extensions)
            .enumerate()
        {
            cx.check(Kind::Work, 1)?;
            if line.index != index
                || output
                    .sources
                    .get(line.source)
                    .is_none_or(|s| building.id(s.element) != Some(s.id))
                || extension.namespace != "zumen.building.drawing"
                || extension.name != "line"
                || !matches!(&extension.target, d::Target::Element(id) if id == &line.drawing_id)
            {
                return Err(Error::Input("drawing_source"));
            }
            let element = model
                .elements
                .get(index)
                .ok_or(Error::Input("drawing_index"))?;
            if extension.basis != d::Basis::Hash(d::Hash::element_checked(element, cx)?) {
                return Err(Error::Input("drawing_modified"));
            }
        }
        settings::style(self, building, &mut output, cx)?;
        let mut decorations = Vec::new();
        decorate::hatches(self, building, &mut output, &mut decorations, cx)?;
        decorate::grids(self, building, &mut output, &mut decorations, cx)?;
        let chain = self.profile.owned_chain(cx)?;
        let hash = self.profile.hash();
        // 一般の図面モデルだけを渡す場合も、採用したプロファイルをたどれる。
        let mut fields = Vec::new();
        reserve(&mut fields, chain.len() * 3 + 1, 80, cx)?;
        cx.check(Kind::DecodedBytes, 32)?;
        fields.push(d::Field {
            key: "hash".into(),
            code: None,
            value: d::FieldValue::Bytes(hash.to_vec()),
        });
        for identity in &chain {
            cx.check(
                Kind::DecodedBytes,
                (identity.id.len() + identity.edition.len()) as u64,
            )?;
            for (key, value) in [
                ("id", identity.id.clone()),
                ("edition", identity.edition.clone()),
            ] {
                fields.push(d::Field {
                    key: key.into(),
                    code: None,
                    value: d::FieldValue::Text(value),
                });
            }
            fields.push(d::Field {
                key: "revision".into(),
                code: None,
                value: d::FieldValue::Handle(identity.revision),
            });
        }
        reserve(&mut output.drawing.extensions, 1, 256, cx)?;
        output.drawing.extensions.push(d::Extension {
            namespace: "zumen.profile.building-drawing".into(),
            name: "profile".into(),
            version: None,
            schema: 1,
            target: d::Target::Document,
            basis: d::Basis::Document,
            fields,
            source: Ref::default(),
        });
        cx.check(Kind::Work, 0)?;
        Ok(Drawing {
            output,
            decorations,
            chain,
            hash,
        })
    }
}

fn length_unit(building: &Building, cx: &mut Cx<'_>) -> Result<f64, Error> {
    cx.check(Kind::Work, building.document.units.len() as u64)?;
    let mut units = building
        .document
        .units
        .iter()
        .filter(|u| u.kind == "LENGTHUNIT");
    let value =
        zumen_building::evaluation::length_scale(units.next().ok_or(Error::Input("drawing_unit"))?)
            .map_err(Error::Geometry)?;
    for unit in units {
        if zumen_building::evaluation::length_scale(unit).map_err(Error::Geometry)? != value {
            return Err(Error::Input("drawing_unit"));
        }
    }
    Ok(value)
}

fn report(
    building: &Building,
    element: ElementRef,
    kind: &str,
    reason: &str,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    cx.check(Kind::Work, 1)?;
    cx.check(Kind::DecodedBytes, 256)?;
    let place = building
        .id(element)
        .map_or_else(Ref::default, |id| Ref::default().with_id(id.to_string()));
    cx.report(
        Diagnostic::new(codes::PROFILE_BUILDING_DRAWING_UNSUPPORTED)
            .with("kind", kind)
            .with("reason", reason)
            .at(place),
    );
    cx.check(Kind::Work, 0)?;
    Ok(())
}
