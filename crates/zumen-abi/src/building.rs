//! IFC の建物モデルの文書と、`building`・`building_elements`・`building_apply`・`ids_check` の op（決定 0207・0217・0219・
//! 0244）。

use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, Diagnostics, Kind, Stop, codes};
use zumen_building::{
    Building, ElementRef, GlobalId, Hash, Id, Literal, Local, MaterialKind, Opening, ParentKind,
    QuantityKind, Relations, SpatialKind, Transform, TypedValue, Unit,
};
use zumen_ids::{Ids, Report};
use zumen_ifc::{
    Edit, FillingKind, Ifc, IfcFile, IfcZip, NewBeam, NewColumn, NewConstituent, NewElement,
    NewFilling, NewGrid, NewGridAxis, NewLayer, NewLayerUsage, NewLiteral, NewMaterial,
    NewMaterialLayer, NewOpening, NewPlacement, NewQuantity, NewSection, NewSlab, NewSpace,
    NewType, NewUnit, NewValue, NewWall, OpeningChange, RootAttribute, SpaceKind, Target,
    TextEncoding, WallChange,
};
use zumen_sdk::{Codec, Lift};

use crate::messages::{
    self, BuildingApplyRequest, BuildingApplyResult, BuildingAttribute, BuildingComponent,
    BuildingEdit, BuildingElement, BuildingElementsRequest, BuildingElementsResult,
    BuildingGridAxis, BuildingLayerDirection, BuildingMaterialChange, BuildingMaterialChangeKind,
    BuildingPlacementChange, BuildingRef, BuildingResult, BuildingSection,
    BuildingSpaceKind, BuildingUnit, BuildingValue, BuildingValueKind, BuildingWallLayer,
    ClassComponent, ClassCount, ClassificationReference, ConnectionLink, IdentityComponent,
    IdsCheckResult, IdsFinding, IdsSpecification, IdsStatus, LevelComponent, MaterialComponent,
    MaterialPart, Measured, Membership, OpeningComponent, ParentComponent, PlacementComponent,
    Property, PropertySet, Quantity, QuantitySet, RelationsComponent, SpatialComponent,
    StyleComponent, ZoneComponent,
};
#[cfg(feature = "bim")]
use crate::plugin;
use crate::session::{Failed, fail, invalid_value, message_of, stopped};

/// 2 つの数の並び（`[x, y]`）。数が 2 つでなければ `None`
fn pair(values: &[f64]) -> Option<[f64; 2]> {
    match values {
        [x, y] => Some([*x, *y]),
        _ => None,
    }
}

/// 3 つの数の並び（`[x, y, z]`）。数が 3 つでなければ `None`
fn triple(values: &[f64]) -> Option<[f64; 3]> {
    match values {
        [x, y, z] => Some([*x, *y, *z]),
        _ => None,
    }
}

/// 材料の層
fn layers(layers: &[BuildingWallLayer]) -> Vec<NewLayer> {
    layers
        .iter()
        .map(|layer| NewLayer {
            material: layer.material.clone(),
            thickness: layer.thickness,
        })
        .collect()
}

/// 矩形の断面
fn section(section: &BuildingSection) -> NewSection {
    NewSection {
        width: section.width,
        depth: section.depth,
    }
}

/// 通り芯の軸。点の数が 2 つでなければ、その点を指して失敗する
fn axes(axes: &[BuildingGridAxis], at: &str, cx: &mut Cx) -> Result<Vec<NewGridAxis>, Failed> {
    let mut out = Vec::with_capacity(axes.len());
    for (i, axis) in axes.iter().enumerate() {
        let start =
            pair(&axis.start).ok_or_else(|| fail(cx, invalid_value(format!("{at}/{i}/start"))))?;
        let end =
            pair(&axis.end).ok_or_else(|| fail(cx, invalid_value(format!("{at}/{i}/end"))))?;
        out.push(NewGridAxis {
            tag: axis.tag.clone(),
            start,
            end,
        });
    }
    Ok(out)
}

/// limit を書かなかったときに返す数
const LIMIT: u64 = 1000;

/// 書き忘れたときに返すコンポーネント
const DEFAULT_COMPONENTS: [BuildingComponent; 2] =
    [BuildingComponent::Class, BuildingComponent::Identity];

/// 開いた建物の文書。IFC の L1 と建物モデルと、入力のバイト列のハッシュ
#[derive(Debug)]
pub(crate) struct Model {
    /// 形式の台帳のキー
    pub(crate) format: &'static str,
    /// IFC の L1。ST-Bridge か、建物の形式のプラグインで開いたなら `None`
    pub(crate) file: Option<IfcFile>,
    pub(crate) building: Building,
    pub(crate) hash: Hash,
    /// ifcZIP で開いたなら、中の IFC-SPF の項目の名前
    pub(crate) entry: Option<String>,
    /// 文書エンジン。`building_apply` で作り、この文書を置き換えるまで持ち続ける（`docs/live.md`）
    #[cfg(feature = "bim")]
    pub(crate) live: Live,
}

/// 建物の文書の文書エンジンの状態。[`Model::file`] と同じ内容を持つ。文書を置き換えると、新しい文書は
/// [`Live::Absent`] から始まるので、ほかの経路で書き換えた文書に古い文書エンジンが残らない
#[cfg(feature = "bim")]
#[derive(Default)]
pub(crate) enum Live {
    /// まだ作っていない
    #[default]
    Absent,
    /// 作れなかったか、今までの経路と同じ読み方で名前を比べられない。この文書では作り直さない
    Refused,
    /// IFC-SPF の文書エンジン。ifcZIP は中の IFC-SPF で作る
    Ifc(Box<zumen_ifc::IfcLive>),
}

#[cfg(feature = "bim")]
impl std::fmt::Debug for Live {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Absent => "Absent",
            Self::Refused => "Refused",
            Self::Ifc(_) => "Ifc",
        })
    }
}

/// IFC-SPF なら読む。IFC-SPF でなければ `None`
pub(crate) fn read_spf(
    bytes: &[u8],
    encoding: Option<&messages::IfcTextEncoding>,
    cx: &mut Cx<'_>,
) -> Option<Result<Model, Failed>> {
    if !Ifc::SIGNATURES.iter().any(|s| s.matches(bytes)) {
        return None;
    }
    Some(
        Ifc::read(bytes, cx)
            .map_err(|_| Failed)
            .and_then(|file| model(file, bytes, None, encoding, cx)),
    )
}

/// ifcZIP なら読む。ZIP でなければ `None`。ZIP の署名はほかの形式とも重なるので、最後に試す
pub(crate) fn read_zip(
    bytes: &[u8],
    encoding: Option<&messages::IfcTextEncoding>,
    cx: &mut Cx<'_>,
) -> Option<Result<Model, Failed>> {
    if !IfcZip::SIGNATURES.iter().any(|s| s.matches(bytes)) {
        return None;
    }
    Some(
        IfcZip::read(bytes, cx)
            .map_err(|_| Failed)
            .and_then(|zipped| {
                let entry = Some(zipped.name().to_owned());
                model(zipped.ifc().clone(), bytes, entry, encoding, cx)
            }),
    )
}

fn model(
    mut file: IfcFile,
    bytes: &[u8],
    entry: Option<String>,
    encoding: Option<&messages::IfcTextEncoding>,
    cx: &mut Cx<'_>,
) -> Result<Model, Failed> {
    file.set_text_encoding(match encoding {
        None | Some(messages::IfcTextEncoding::Auto) => TextEncoding::Auto,
        Some(messages::IfcTextEncoding::Iso88591) => TextEncoding::Iso8859_1,
        Some(messages::IfcTextEncoding::ShiftJis) => TextEncoding::ShiftJis,
    });
    let building = file.lift(cx).map_err(|_| Failed)?;
    let hash = Hash::document_checked(bytes, cx).map_err(|stop| stopped(cx, stop))?;
    Ok(Model {
        format: Ifc::FORMAT,
        file: Some(file),
        building,
        hash,
        entry,
        #[cfg(feature = "bim")]
        live: Live::Absent,
    })
}

/// ST-Bridge なら読み、建物モデルへ移す（#400）。ST-Bridge でなければ `None`。L1 を持たないので書き戻さない
#[cfg(feature = "bim")]
pub(crate) fn read_stbridge(bytes: &[u8], cx: &mut Cx<'_>) -> Option<Result<Model, Failed>> {
    use zumen_stbridge::Stb;
    if !Stb::SIGNATURES.iter().any(|s| s.matches(bytes)) {
        return None;
    }
    Some(
        Stb::read(bytes, cx)
            .and_then(|file| file.lift(cx))
            .map_err(|_| Failed)
            .and_then(|building| {
                let hash = Hash::document_checked(bytes, cx).map_err(|stop| stopped(cx, stop))?;
                Ok(Model {
                    format: Stb::FORMAT,
                    file: None,
                    building,
                    hash,
                    entry: None,
                    live: Live::Absent,
                })
            }),
    )
}

/// 登録した建物の形式のプラグインのどれかに合えば読む。どれにも合わなければ `None`
#[cfg(feature = "bim")]
pub(crate) fn read_plugin(bytes: &[u8], cx: &mut Cx<'_>) -> Option<Result<Model, Failed>> {
    let (format, read) = plugin::installed()
        .iter()
        .find_map(|plugin| match plugin.read {
            plugin::Reader::Building(read) if plugin.registration.matches(bytes) => {
                Some((plugin.registration.format, read))
            }
            _ => None,
        })?;
    Some(read(bytes, cx).map_err(|_| Failed).and_then(|building| {
        let hash = Hash::document_checked(bytes, cx).map_err(|stop| stopped(cx, stop))?;
        Ok(Model {
            format,
            file: None,
            building,
            hash,
            entry: None,
            live: Live::Absent,
        })
    }))
}

/// 書き戻した文書と結果と、書き戻したファイル
pub(crate) type Updated = (Model, BuildingApplyResult, Vec<u8>);

/// 確かめ終えた変更の一覧。L1 の変更と、変える要素（結果の並び）
pub(crate) struct Prepared {
    pub(crate) edits: Vec<Edit>,
    targets: Vec<Outcome>,
}

/// 変更 1 つの結果で返す要素
#[derive(Clone, Copy)]
enum Outcome {
    /// 文書にあった要素。ID は書き戻しても変わらない
    Existing(ElementRef),
    /// 足した空間か要素の GlobalId
    Added(GlobalId),
    /// 除いた要素か、要素でないもの（単位）を変えた。ID を返さない
    Removed,
}

/// 確かめ終えた変更を、今までの経路（L1 の [`zumen_ifc::apply`]）で当て、書き戻した文書と結果と、書き戻した
/// ファイルを返す（決定 0244）。元の文書は変えない
pub(crate) fn applied(
    model: &Model,
    prepared: &Prepared,
    cx: &mut Cx<'_>,
) -> Result<Updated, Failed> {
    let Some(source) = &model.file else {
        return Err(Failed);
    };
    let applied =
        zumen_ifc::apply(source, &prepared.edits, &model.hash.0, cx).map_err(|_| Failed)?;
    finish(model, applied.bytes, prepared, cx)
}

/// 変更の一覧を確かめ、L1 の変更にする。文書と要素のハッシュ・値の形・要素があるかを確かめ、当てられない
/// 変更があれば診断して失敗する。文書の種類と予算のほかは、文書を読むだけ
pub(crate) fn prepare(
    model: &Model,
    request: &BuildingApplyRequest,
    cx: &mut Cx<'_>,
) -> Result<Prepared, Failed> {
    if model.file.is_none() {
        // ST-Bridge とプラグインの文書は書き戻せない（決定 0243、#400）
        return Err(fail(
            cx,
            Diagnostic::new(codes::CALL_DOCUMENT_UNSUPPORTED)
                .with("name", request.doc.as_str())
                .with("op", "building_apply"),
        ));
    }
    if let Some(text) = &request.hash {
        let hash = text
            .parse::<Hash>()
            .map_err(|()| fail(cx, invalid_value("/hash")))?;
        if hash != model.hash {
            return Err(fail(cx, Diagnostic::new(codes::CHANGE_DOCUMENT_STALE)));
        }
    }
    // 足す変更の、L1 の足した順の位置と GlobalId。前の変更の added から引く
    let mut slots: Vec<Option<(usize, GlobalId)>> = Vec::with_capacity(request.edits.len());
    let mut edits = Vec::with_capacity(request.edits.len());
    let mut targets = Vec::with_capacity(request.edits.len());
    let mut rejected = false;
    for (index, edit) in request.edits.iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        let at = format!("/edits/{index}");
        let added = slots.iter().flatten().count();
        let given = match edit {
            BuildingEdit::AddSpace(edit) => Some(edit.global_id.as_deref()),
            BuildingEdit::AddElement(edit) => Some(edit.global_id.as_deref()),
            BuildingEdit::AddWall(edit) => Some(edit.global_id.as_deref()),
            BuildingEdit::AddSlab(edit) => Some(edit.global_id.as_deref()),
            BuildingEdit::AddColumn(edit) => Some(edit.global_id.as_deref()),
            BuildingEdit::AddBeam(edit) => Some(edit.global_id.as_deref()),
            BuildingEdit::AddOpening(edit) => Some(edit.global_id.as_deref()),
            BuildingEdit::AddDoor(edit) => Some(edit.global_id.as_deref()),
            BuildingEdit::AddWindow(edit) => Some(edit.global_id.as_deref()),
            BuildingEdit::AddGrid(edit) => Some(edit.global_id.as_deref()),
            BuildingEdit::AddType(edit) => Some(edit.global_id.as_deref()),
            _ => None,
        };
        let slot = match given {
            Some(Some(text)) => Some(
                GlobalId::parse(text)
                    .ok_or_else(|| fail(cx, invalid_value(format!("{at}/global_id"))))?,
            ),
            Some(None) => Some(minted(model, index)),
            None => None,
        };
        slots.push(slot.map(|global_id| (added, global_id)));
        let refs = Refs {
            model,
            slots: &slots,
            index,
        };
        let resolved = match edit {
            BuildingEdit::Property(_) | BuildingEdit::Attribute(_) | BuildingEdit::Quantity(_) => {
                old_edit(model, edit, &at, cx)?
            }
            BuildingEdit::AddType(edit) => {
                let global_id = slot.ok_or(Failed)?;
                Some((
                    Edit::ElementType(NewType {
                        class: edit.class.clone(),
                        predefined: edit.predefined.clone(),
                        element_type: edit.element_type.clone(),
                        name: edit.name.clone(),
                        global_id: Some(global_id),
                    }),
                    Outcome::Added(global_id),
                ))
            }
            BuildingEdit::AddUnit(edit) => Some((
                Edit::Unit(NewUnit {
                    kind: edit.unit.clone(),
                    prefix: edit.prefix.clone(),
                    name: edit.name.clone(),
                }),
                Outcome::Removed,
            )),
            BuildingEdit::Material(edit) => {
                // 材料の形は、要素を引く前に確かめる
                let material = new_material(&edit.material)
                    .ok_or_else(|| fail(cx, invalid_value(format!("{at}/material"))))?;
                let element = refs.resolve(&edit.element, &format!("{at}/element"), cx)?;
                element.map(|(element, outcome)| (Edit::Material { element, material }, outcome))
            }
            BuildingEdit::AddSpace(edit) => {
                let parent = refs.resolve(&edit.parent, &format!("{at}/parent"), cx)?;
                let placement = refs.placement(edit.placement.as_ref(), &at, cx)?;
                let global_id = slot.ok_or(Failed)?;
                parent.zip(placement).map(|(parent, placement)| {
                    (
                        Edit::Space(NewSpace {
                            kind: match edit.space {
                                BuildingSpaceKind::Site => SpaceKind::Site,
                                BuildingSpaceKind::Building => SpaceKind::Building,
                                BuildingSpaceKind::Storey => SpaceKind::Storey,
                            },
                            name: edit.name.clone(),
                            parent: parent.0,
                            placement,
                            elevation: edit.elevation,
                            global_id: Some(global_id),
                        }),
                        Outcome::Added(global_id),
                    )
                })
            }
            BuildingEdit::AddElement(edit) => {
                let container = match &edit.container {
                    Some(r) => refs
                        .resolve(r, &format!("{at}/container"), cx)?
                        .map(|(target, _)| Some(target)),
                    None => Some(None),
                };
                let placement = refs.placement(edit.placement.as_ref(), &at, cx)?;
                let global_id = slot.ok_or(Failed)?;
                container.zip(placement).map(|(container, placement)| {
                    (
                        Edit::Element(NewElement {
                            class: edit.class.clone(),
                            predefined: edit.predefined.clone(),
                            name: edit.name.clone(),
                            container,
                            placement,
                            global_id: Some(global_id),
                        }),
                        Outcome::Added(global_id),
                    )
                })
            }
            BuildingEdit::AddWall(edit) => {
                let storey = refs.resolve(&edit.storey, &format!("{at}/storey"), cx)?;
                let start = pair(&edit.start)
                    .ok_or_else(|| fail(cx, invalid_value(format!("{at}/start"))))?;
                let end =
                    pair(&edit.end).ok_or_else(|| fail(cx, invalid_value(format!("{at}/end"))))?;
                let global_id = slot.ok_or(Failed)?;
                storey.map(|(storey, _)| {
                    (
                        Edit::Wall(NewWall {
                            storey,
                            start,
                            end,
                            height: edit.height,
                            layers: layers(&edit.layers),
                            offset: edit.offset,
                            name: edit.name.clone(),
                            global_id: Some(global_id),
                        }),
                        Outcome::Added(global_id),
                    )
                })
            }
            BuildingEdit::AddSlab(edit) => {
                let storey = refs.resolve(&edit.storey, &format!("{at}/storey"), cx)?;
                let mut outline = Vec::with_capacity(edit.outline.len());
                for (i, point) in edit.outline.iter().enumerate() {
                    outline.push(
                        pair(point)
                            .ok_or_else(|| fail(cx, invalid_value(format!("{at}/outline/{i}"))))?,
                    );
                }
                let global_id = slot.ok_or(Failed)?;
                storey.map(|(storey, _)| {
                    (
                        Edit::Slab(NewSlab {
                            storey,
                            outline,
                            layers: layers(&edit.layers),
                            elevation: edit.elevation,
                            name: edit.name.clone(),
                            global_id: Some(global_id),
                        }),
                        Outcome::Added(global_id),
                    )
                })
            }
            BuildingEdit::AddColumn(edit) => {
                let storey = refs.resolve(&edit.storey, &format!("{at}/storey"), cx)?;
                let position = pair(&edit.position)
                    .ok_or_else(|| fail(cx, invalid_value(format!("{at}/position"))))?;
                let direction = match &edit.direction {
                    Some(direction) => Some(
                        pair(direction)
                            .ok_or_else(|| fail(cx, invalid_value(format!("{at}/direction"))))?,
                    ),
                    None => None,
                };
                let global_id = slot.ok_or(Failed)?;
                storey.map(|(storey, _)| {
                    (
                        Edit::Column(NewColumn {
                            storey,
                            position,
                            base: edit.base,
                            height: edit.height,
                            section: section(&edit.section),
                            direction,
                            material: edit.material.clone(),
                            name: edit.name.clone(),
                            global_id: Some(global_id),
                        }),
                        Outcome::Added(global_id),
                    )
                })
            }
            BuildingEdit::AddBeam(edit) => {
                let storey = refs.resolve(&edit.storey, &format!("{at}/storey"), cx)?;
                let start = triple(&edit.start)
                    .ok_or_else(|| fail(cx, invalid_value(format!("{at}/start"))))?;
                let end = triple(&edit.end)
                    .ok_or_else(|| fail(cx, invalid_value(format!("{at}/end"))))?;
                let global_id = slot.ok_or(Failed)?;
                storey.map(|(storey, _)| {
                    (
                        Edit::Beam(NewBeam {
                            storey,
                            start,
                            end,
                            section: section(&edit.section),
                            material: edit.material.clone(),
                            name: edit.name.clone(),
                            global_id: Some(global_id),
                        }),
                        Outcome::Added(global_id),
                    )
                })
            }
            BuildingEdit::AddOpening(edit) => {
                let host = refs.resolve(&edit.host, &format!("{at}/host"), cx)?;
                let global_id = slot.ok_or(Failed)?;
                host.map(|(host, _)| {
                    (
                        Edit::Opening(NewOpening {
                            host,
                            distance: edit.distance,
                            sill: edit.sill,
                            width: edit.width,
                            height: edit.height,
                            name: edit.name.clone(),
                            global_id: Some(global_id),
                        }),
                        Outcome::Added(global_id),
                    )
                })
            }
            BuildingEdit::AddDoor(edit) => {
                let opening = refs.resolve(&edit.opening, &format!("{at}/opening"), cx)?;
                let global_id = slot.ok_or(Failed)?;
                opening.map(|(opening, _)| {
                    (
                        Edit::Filling(NewFilling {
                            opening,
                            kind: FillingKind::Door,
                            width: edit.width,
                            height: edit.height,
                            name: edit.name.clone(),
                            global_id: Some(global_id),
                        }),
                        Outcome::Added(global_id),
                    )
                })
            }
            BuildingEdit::AddWindow(edit) => {
                let opening = refs.resolve(&edit.opening, &format!("{at}/opening"), cx)?;
                let global_id = slot.ok_or(Failed)?;
                opening.map(|(opening, _)| {
                    (
                        Edit::Filling(NewFilling {
                            opening,
                            kind: FillingKind::Window,
                            width: edit.width,
                            height: edit.height,
                            name: edit.name.clone(),
                            global_id: Some(global_id),
                        }),
                        Outcome::Added(global_id),
                    )
                })
            }
            BuildingEdit::AddGrid(edit) => {
                let storey = refs.resolve(&edit.storey, &format!("{at}/storey"), cx)?;
                let u = axes(&edit.u, &format!("{at}/u"), cx)?;
                let v = axes(&edit.v, &format!("{at}/v"), cx)?;
                let global_id = slot.ok_or(Failed)?;
                storey.map(|(storey, _)| {
                    (
                        Edit::Grid(NewGrid {
                            storey,
                            u,
                            v,
                            elevation: edit.elevation,
                            name: edit.name.clone(),
                            global_id: Some(global_id),
                        }),
                        Outcome::Added(global_id),
                    )
                })
            }
            BuildingEdit::Elevation(edit) => {
                let storey = refs.resolve(&edit.storey, &format!("{at}/storey"), cx)?;
                storey.map(|(storey, outcome)| {
                    (
                        Edit::Elevation {
                            storey,
                            elevation: edit.elevation,
                        },
                        outcome,
                    )
                })
            }
            BuildingEdit::WallShape(edit) => {
                let wall = refs.resolve(&edit.wall, &format!("{at}/wall"), cx)?;
                let start = pair(&edit.start)
                    .ok_or_else(|| fail(cx, invalid_value(format!("{at}/start"))))?;
                let end =
                    pair(&edit.end).ok_or_else(|| fail(cx, invalid_value(format!("{at}/end"))))?;
                wall.map(|(wall, outcome)| {
                    (
                        Edit::WallShape(WallChange {
                            wall,
                            start,
                            end,
                            height: edit.height,
                            layers: layers(&edit.layers),
                            offset: edit.offset,
                        }),
                        outcome,
                    )
                })
            }
            BuildingEdit::OpeningShape(edit) => {
                let opening = refs.resolve(&edit.opening, &format!("{at}/opening"), cx)?;
                opening.map(|(opening, outcome)| {
                    (
                        Edit::OpeningShape(OpeningChange {
                            opening,
                            distance: edit.distance,
                            sill: edit.sill,
                            width: edit.width,
                            height: edit.height,
                        }),
                        outcome,
                    )
                })
            }
            BuildingEdit::Remove(edit) => {
                let element = BuildingRef {
                    id: Some(edit.id.clone()),
                    hash: Some(edit.hash.clone()),
                    added: None,
                };
                refs.resolve(&element, &at, cx)?
                    .map(|(element, _)| (Edit::Remove { element }, Outcome::Removed))
            }
            BuildingEdit::Place(edit) => {
                let element = refs.resolve(&edit.element, &format!("{at}/element"), cx)?;
                let placement = refs.placement(Some(&edit.placement), &at, cx)?;
                element
                    .zip(placement.flatten())
                    .map(|((element, outcome), placement)| {
                        (Edit::Place { element, placement }, outcome)
                    })
            }
            BuildingEdit::Contain(edit) => {
                let element = refs.resolve(&edit.element, &format!("{at}/element"), cx)?;
                let space = refs.resolve(&edit.space, &format!("{at}/space"), cx)?;
                element.zip(space).map(|((element, outcome), (space, _))| {
                    (Edit::Contain { element, space }, outcome)
                })
            }
            BuildingEdit::Type(edit) => {
                let element = refs.resolve(&edit.element, &format!("{at}/element"), cx)?;
                let type_of = match &edit.element_type {
                    Some(r) => refs
                        .resolve(r, &format!("{at}/element_type"), cx)?
                        .map(|(target, _)| Some(target)),
                    None => Some(None),
                };
                element
                    .zip(type_of)
                    .map(|((element, outcome), type_of)| (Edit::Type { element, type_of }, outcome))
            }
            BuildingEdit::Void(edit) => {
                let host = refs.resolve(&edit.host, &format!("{at}/host"), cx)?;
                let opening = refs.resolve(&edit.opening, &format!("{at}/opening"), cx)?;
                host.zip(opening)
                    .map(|((host, outcome), (opening, _))| (Edit::Void { host, opening }, outcome))
            }
            BuildingEdit::Fill(edit) => {
                let opening = refs.resolve(&edit.opening, &format!("{at}/opening"), cx)?;
                let filler = refs.resolve(&edit.filler, &format!("{at}/filler"), cx)?;
                opening
                    .zip(filler)
                    .map(|((opening, outcome), (filler, _))| {
                        (Edit::Fill { opening, filler }, outcome)
                    })
            }
        };
        match resolved {
            Some((edit, outcome)) => {
                edits.push(edit);
                targets.push(outcome);
            }
            None => rejected = true,
        }
    }
    if rejected {
        return Err(Failed);
    }
    Ok(Prepared { edits, targets })
}

/// プロパティと属性の変更を確かめ、L1 の変更にする。要素がないか古ければ診断して `None`
fn old_edit(
    model: &Model,
    edit: &BuildingEdit,
    at: &str,
    cx: &mut Cx<'_>,
) -> Result<Option<(Edit, Outcome)>, Failed> {
    let (id, hash) = match edit {
        BuildingEdit::Property(edit) => (&edit.id, &edit.hash),
        BuildingEdit::Attribute(edit) => (&edit.id, &edit.hash),
        BuildingEdit::Quantity(edit) => (&edit.id, &edit.hash),
        _ => return Err(Failed),
    };
    let expected = hash
        .parse::<Hash>()
        .map_err(|()| fail(cx, invalid_value(format!("{at}/hash"))))?;
    // 値の形は、要素を引く前に確かめる
    let value = match edit {
        BuildingEdit::Property(edit) => Some(
            new_value(&edit.value).ok_or_else(|| fail(cx, invalid_value(format!("{at}/value"))))?,
        ),
        _ => None,
    };
    let Some((instance, element)) = current(model, id, expected, cx)? else {
        return Ok(None);
    };
    let edit = match (edit, value) {
        (BuildingEdit::Property(edit), Some(value)) => Edit::Property {
            element: instance,
            set: edit.set.clone(),
            name: edit.name.clone(),
            value,
        },
        (BuildingEdit::Quantity(edit), _) => Edit::Quantity {
            element: instance,
            set: edit.set.clone(),
            method: edit.method.clone(),
            quantity: NewQuantity {
                name: edit.name.clone(),
                kind: quantity_kind(edit.quantity),
                value: edit.value,
                formula: edit.formula.clone(),
            },
        },
        (BuildingEdit::Attribute(edit), _) => Edit::Attribute {
            element: instance,
            attribute: match edit.attribute {
                BuildingAttribute::Name => RootAttribute::Name,
                BuildingAttribute::Description => RootAttribute::Description,
            },
            value: edit.value.clone(),
        },
        _ => return Err(Failed),
    };
    Ok(Some((edit, Outcome::Existing(element))))
}

/// ID の要素を引き、ハッシュが `expected` と同じか確かめる。インスタンスの番号と要素を返す。
/// なければ・古ければ診断して `None`
fn current(
    model: &Model,
    id: &str,
    expected: Hash,
    cx: &mut Cx<'_>,
) -> Result<Option<(u64, ElementRef)>, Failed> {
    let Some(element) = id.parse::<Id>().ok().and_then(|id| model.building.find(id)) else {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", id));
        return Ok(None);
    };
    let now =
        Hash::element_checked(&model.building, element, cx).map_err(|stop| stopped(cx, stop))?;
    if now != Some(expected) {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", id));
        return Ok(None);
    }
    let instance = model
        .building
        .element(element)
        .and_then(|e| e.source().instance)
        .unwrap_or_default();
    Ok(Some((instance, element)))
}

/// 呼び出し元が GlobalId を書かずに足す空間か要素の GlobalId。文書のハッシュと変更の位置から決める
fn minted(model: &Model, index: usize) -> GlobalId {
    let mut input = model.hash.0.to_vec();
    input.extend_from_slice(b"building_apply/");
    input.extend_from_slice(&(index as u64).to_le_bytes());
    let Hash(bytes) = Hash::document(&input);
    GlobalId::from_uuid(bytes)
}

/// 変更が指す要素を引くための、文書と、前の変更で足したもの
struct Refs<'a> {
    model: &'a Model,
    slots: &'a [Option<(usize, GlobalId)>],
    index: usize,
}

impl Refs<'_> {
    /// `r` が指す要素を L1 の指し方にする。形が違えば失敗し、要素がないか古ければ診断して `None`
    fn resolve(
        &self,
        r: &BuildingRef,
        at: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Option<(Target, Outcome)>, Failed> {
        match (&r.id, &r.hash, r.added) {
            (Some(id), Some(hash), None) => {
                let expected = hash
                    .parse::<Hash>()
                    .map_err(|()| fail(cx, invalid_value(format!("{at}/hash"))))?;
                Ok(
                    current(self.model, id, expected, cx)?.map(|(instance, element)| {
                        (Target::Instance(instance), Outcome::Existing(element))
                    }),
                )
            }
            (None, None, Some(added)) => {
                // 前にある足す変更だけを指せる
                let slot = usize::try_from(added)
                    .ok()
                    .filter(|&n| n < self.index)
                    .and_then(|n| self.slots.get(n).copied().flatten());
                let (position, global_id) =
                    slot.ok_or_else(|| fail(cx, invalid_value(format!("{at}/added"))))?;
                Ok(Some((Target::Added(position), Outcome::Added(global_id))))
            }
            _ => Err(fail(cx, invalid_value(at))),
        }
    }

    /// 配置を L1 の配置にする。書かなければ `Some(None)`。行列の形が違えば失敗し、親の要素がないか古ければ
    /// 診断して `None`
    fn placement(
        &self,
        placement: Option<&BuildingPlacementChange>,
        at: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Option<NewPlacement>>, Failed> {
        let Some(placement) = placement else {
            return Ok(Some(None));
        };
        let at = format!("{at}/placement");
        let local = matrix(&placement.matrix)
            .ok_or_else(|| fail(cx, invalid_value(format!("{at}/matrix"))))?;
        let relative_to = match &placement.relative_to {
            Some(r) => match self.resolve(r, &format!("{at}/relative_to"), cx)? {
                Some((target, _)) => Some(target),
                None => return Ok(None),
            },
            None => None,
        };
        Ok(Some(Some(NewPlacement { relative_to, local })))
    }
}

/// 3 行 4 列の有限の数を変換にする。形が違えば `None`
fn matrix(rows: &[Vec<f64>]) -> Option<Transform> {
    let mut out = [[0.0; 4]; 3];
    if rows.len() != out.len() {
        return None;
    }
    for (row, values) in out.iter_mut().zip(rows) {
        if values.len() != row.len() || values.iter().any(|v| !v.is_finite()) {
            return None;
        }
        row.copy_from_slice(values);
    }
    Some(Transform(out))
}

/// 書き戻した IFC-SPF のバイト列 `spf` から、書き戻した文書と結果と、書き戻したファイルを作る。ifcZIP なら
/// アーカイブに入れる。文書エンジンで当てたときも、今までの経路と同じこの手順で作る
pub(crate) fn finish(
    model: &Model,
    spf: Vec<u8>,
    prepared: &Prepared,
    cx: &mut Cx<'_>,
) -> Result<Updated, Failed> {
    let encoding = model
        .file
        .as_ref()
        .map(IfcFile::text_encoding)
        .unwrap_or_default();
    // 書き戻したものを読み直す。読んだときの診断は、開いたときに返したので捨てる
    let mut scratch = Diagnostics::new();
    let reread = {
        let mut inner = Cx {
            budget: &mut *cx.budget,
            diag: &mut scratch,
            cancel: cx.cancel,
        };
        Ifc::read(&spf, &mut inner).and_then(|mut file| {
            file.set_text_encoding(encoding);
            file.lift(&mut inner).map(|building| (file, building))
        })
    };
    let (file, building) = match reread {
        Ok(read) => read,
        Err(zumen_sdk::Error::Stopped(stop)) => return Err(stopped(cx, stop)),
        // 書いたものを読めないのは不具合
        Err(_) => return Err(fail(cx, Diagnostic::new(codes::INTERNAL_PANIC_CAUGHT))),
    };
    let bytes = match &model.entry {
        Some(entry) => zumen_zip::write(&[(entry.as_str(), &spf)])
            .ok_or_else(|| fail(cx, Diagnostic::new(codes::INTERNAL_PANIC_CAUGHT)))?,
        None => spf,
    };
    let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
    let updated = Model {
        format: model.format,
        file: Some(file),
        building,
        hash,
        entry: model.entry.clone(),
        #[cfg(feature = "bim")]
        live: Live::Absent,
    };
    let mut results = Vec::with_capacity(prepared.targets.len());
    for &outcome in &prepared.targets {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        // 変えた要素の ID は変わらない。足した空間と要素は、GlobalId の ID で引く
        let id = match outcome {
            Outcome::Existing(element) => model.building.id(element),
            Outcome::Added(global_id) => Some(Id::Global(global_id)),
            Outcome::Removed => None,
        };
        let after = id.and_then(|id| updated.building.find(id));
        let hash = match after {
            Some(after) => Hash::element_checked(&updated.building, after, cx)
                .map_err(|stop| stopped(cx, stop))?,
            None => None,
        };
        results.push(messages::Applied {
            id: after.and(id).map(|id| id.to_string()),
            hash: hash.map(|hash| hash.to_string()),
        });
    }
    let result = BuildingApplyResult {
        applied: results,
        hash: updated.hash.to_string(),
    };
    Ok((updated, result, bytes))
}

/// API の数量の種類を、建物モデルの種類にする
fn quantity_kind(kind: messages::QuantityKind) -> QuantityKind {
    match kind {
        messages::QuantityKind::Length => QuantityKind::Length,
        messages::QuantityKind::Area => QuantityKind::Area,
        messages::QuantityKind::Volume => QuantityKind::Volume,
        messages::QuantityKind::Count => QuantityKind::Count,
        messages::QuantityKind::Weight => QuantityKind::Weight,
        messages::QuantityKind::Time => QuantityKind::Time,
    }
}

/// API の材料の構成を、結ぶ材料の構成にする。種類に合うフィールドがないか、ほかの種類のフィールドがあれば
/// `None`。空の並びは L1 が断る
fn new_material(change: &BuildingMaterialChange) -> Option<NewMaterial> {
    let BuildingMaterialChange {
        kind,
        name,
        names,
        layers,
        usage,
        constituents,
    } = change;
    let given = [
        name.is_some(),
        names.is_some(),
        layers.is_some() || usage.is_some(),
        constituents.is_some(),
    ];
    let wanted = match kind {
        BuildingMaterialChangeKind::Single => 0,
        BuildingMaterialChangeKind::List => 1,
        BuildingMaterialChangeKind::Layers => 2,
        BuildingMaterialChangeKind::Constituents => 3,
    };
    if given.iter().enumerate().any(|(n, &on)| on != (n == wanted)) {
        return None;
    }
    Some(match kind {
        BuildingMaterialChangeKind::Single => NewMaterial::Single(name.clone()?),
        BuildingMaterialChangeKind::List => NewMaterial::List(names.clone()?),
        BuildingMaterialChangeKind::Layers => NewMaterial::Layers {
            layers: layers
                .as_deref()?
                .iter()
                .map(|layer| NewMaterialLayer {
                    material: layer.material.clone(),
                    thickness: layer.thickness,
                    name: layer.name.clone(),
                })
                .collect(),
            usage: usage.as_ref().map(|usage| NewLayerUsage {
                direction: match usage.direction {
                    BuildingLayerDirection::Axis1 => "AXIS1",
                    BuildingLayerDirection::Axis2 => "AXIS2",
                    BuildingLayerDirection::Axis3 => "AXIS3",
                }
                .to_owned(),
                negative: usage.negative.unwrap_or(false),
                offset: usage.offset,
            }),
        },
        BuildingMaterialChangeKind::Constituents => NewMaterial::Constituents(
            constituents
                .as_deref()?
                .iter()
                .map(|part| NewConstituent {
                    material: part.material.clone(),
                    name: part.name.clone(),
                })
                .collect(),
        ),
    })
}

/// API の値を、書く値にする。書けない種類か、フィールドが足りなければ `None`
fn new_value(value: &BuildingValue) -> Option<NewValue> {
    let literal = match value.kind {
        BuildingValueKind::Real => NewLiteral::Real(value.number.filter(|n| n.is_finite())?),
        BuildingValueKind::Integer => match (value.number, &value.text) {
            (Some(number), None) => {
                // 2^53 − 1 までの整数だけを数で受ける
                (number.fract() == 0.0 && number.abs() <= 9_007_199_254_740_991.0)
                    .then_some(NewLiteral::Integer(number as i64))?
            }
            (None, Some(text)) => NewLiteral::Integer(text.parse().ok()?),
            _ => return None,
        },
        BuildingValueKind::Text => NewLiteral::Text(value.text.clone()?),
        BuildingValueKind::Boolean => NewLiteral::Boolean(value.boolean?),
        BuildingValueKind::Logical => NewLiteral::Logical(value.boolean),
        _ => return None,
    };
    Some(NewValue {
        type_name: value.type_name.clone()?,
        literal,
    })
}

impl Model {
    fn id(&self, element: ElementRef) -> String {
        self.building
            .id(element)
            .map(|id| id.to_string())
            .unwrap_or_default()
    }

    fn ids(&self, elements: &[ElementRef]) -> Vec<String> {
        elements.iter().map(|e| self.id(*e)).collect()
    }

    /// 要素の ID とコンポーネントのハッシュ。要素の書いた順
    pub(crate) fn hashes(&self, cx: &mut Cx<'_>) -> Result<BTreeMap<String, String>, Failed> {
        let mut hashes = BTreeMap::new();
        for index in 0..self.building.elements().len() {
            let element = ElementRef(u32::try_from(index).unwrap_or(u32::MAX));
            if let Some(hash) = Hash::element_checked(&self.building, element, cx)
                .map_err(|stop| stopped(cx, stop))?
            {
                hashes.insert(self.id(element), hash.to_string());
            }
        }
        Ok(hashes)
    }
}

/// 建物モデルの要約
pub(crate) fn summary(model: &Model) -> BuildingResult {
    let document = &model.building.document;
    let mut classes: BTreeMap<&str, u64> = BTreeMap::new();
    for (_, class) in model.building.components.class.iter() {
        *classes.entry(class.name.as_str()).or_default() += 1;
    }
    BuildingResult {
        // プラグインの文書は、建物モデルのヘッダーのスキーマの名前
        schema: match &model.file {
            Some(file) => String::from_utf8_lossy(file.schema()).into_owned(),
            None => model
                .building
                .document
                .header
                .schemas
                .first()
                .cloned()
                .unwrap_or_default(),
        },
        hash: model.hash.to_string(),
        project: document.project.map(|p| model.id(p)),
        project_name: document.project_name.clone(),
        units: document.units.iter().map(unit).collect(),
        elements: model.building.elements().len() as u64,
        classes: classes
            .into_iter()
            .map(|(name, count)| ClassCount {
                name: name.to_string(),
                count,
            })
            .collect(),
    }
}

/// 要素を選び、選んだコンポーネントとともに一部を返す
pub(crate) fn elements(
    model: &Model,
    request: &BuildingElementsRequest,
    cx: &mut Cx<'_>,
) -> Result<BuildingElementsResult, Failed> {
    let components = request.components.as_deref().unwrap_or(&DEFAULT_COMPONENTS);
    for (index, component) in components.iter().enumerate() {
        if components.iter().take(index).any(|c| c == component) {
            return Err(fail(cx, invalid_value(format!("/components/{index}"))));
        }
    }
    if components.is_empty() {
        return Err(fail(cx, invalid_value("/components")));
    }
    let building = &model.building;
    let chosen: Vec<ElementRef> = match &request.ids {
        Some(ids) => ids
            .iter()
            .filter_map(|id| id.parse::<Id>().ok().and_then(|id| building.find(id)))
            .collect(),
        None => (0..building.elements().len())
            .map(|index| ElementRef(u32::try_from(index).unwrap_or(u32::MAX)))
            .collect(),
    };
    let chosen: Vec<ElementRef> = match &request.class {
        Some(name) => chosen
            .into_iter()
            .filter(|e| {
                building
                    .components
                    .class
                    .get(*e)
                    .is_some_and(|class| class.name.eq_ignore_ascii_case(name))
            })
            .collect(),
        None => chosen,
    };
    let relations = if components.contains(&BuildingComponent::Relations) {
        Some(building.relations(cx).map_err(|stop| stopped(cx, stop))?)
    } else {
        None
    };
    let total = chosen.len() as u64;
    let start = request.offset.unwrap_or(0);
    let end = start.saturating_add(request.limit.unwrap_or(LIMIT));
    let mut elements = Vec::new();
    for element in chosen
        .into_iter()
        .skip(usize::try_from(start).unwrap_or(usize::MAX))
        .take(usize::try_from(end - start).unwrap_or(usize::MAX))
    {
        elements.push(
            self::element(model, element, components, relations.as_ref(), cx)
                .map_err(|stop| stopped(cx, stop))?,
        );
    }
    Ok(BuildingElementsResult {
        elements,
        total,
        next_offset: (end < total).then_some(end),
    })
}

fn element(
    model: &Model,
    element: ElementRef,
    components: &[BuildingComponent],
    relations: Option<&Relations>,
    cx: &mut Cx<'_>,
) -> Result<BuildingElement, Stop> {
    let building = &model.building;
    let c = &building.components;
    let has = |component| components.contains(&component);
    let id = |e: &ElementRef| model.id(*e);
    let mut out = BuildingElement {
        id: model.id(element),
        hash: Hash::element_checked(building, element, cx)?
            .map(|hash| hash.to_string())
            .unwrap_or_default(),
        identity: None,
        class: None,
        classification: None,
        spatial: None,
        parent: None,
        placement: None,
        type_of: None,
        properties: None,
        quantities: None,
        material: None,
        opening: None,
        group: None,
        connection: None,
        level: None,
        zone: None,
        style: None,
        relations: None,
    };
    if has(BuildingComponent::Identity) {
        out.identity = c.identity.get(element).map(|i| IdentityComponent {
            name: i.name.clone(),
            description: i.description.clone(),
            tag: i.tag.clone(),
            long_name: i.long_name.clone(),
        });
    }
    if has(BuildingComponent::Class) {
        out.class = c.class.get(element).map(|class| ClassComponent {
            name: class.name.clone(),
            predefined: class.predefined.clone(),
            user_defined: class.user_defined.clone(),
            original: class.original.clone(),
        });
    }
    if has(BuildingComponent::Classification) {
        out.classification = c.classification.get(element).map(|found| {
            found
                .references
                .iter()
                .map(|r| ClassificationReference {
                    system: r.system.clone(),
                    edition: r.edition.clone(),
                    code: r.code.clone(),
                    name: r.name.clone(),
                })
                .collect()
        });
    }
    if has(BuildingComponent::Spatial) {
        out.spatial = c.spatial.get(element).map(|s| SpatialComponent {
            kind: match &s.kind {
                SpatialKind::Site => "site".to_string(),
                SpatialKind::Building => "building".to_string(),
                SpatialKind::Storey => "storey".to_string(),
                SpatialKind::Space => "space".to_string(),
                SpatialKind::SpatialZone => "spatial_zone".to_string(),
                SpatialKind::Other(name) => name.clone(),
            },
            elevation: s.elevation.as_ref().map(measured),
            ref_elevation: s.ref_elevation.as_ref().map(measured),
        });
    }
    if has(BuildingComponent::Parent) {
        out.parent = c.parent.get(element).map(|p| ParentComponent {
            of: p.of.as_ref().map(|m| Membership {
                target: id(&m.target),
                kind: match m.kind {
                    ParentKind::Aggregates => messages::ParentKind::Aggregates,
                    ParentKind::Nests => messages::ParentKind::Nests,
                    ParentKind::Contained => messages::ParentKind::Contained,
                },
                order: m.order.map(u64::from),
            }),
            referenced_in: model.ids(&p.referenced_in),
        });
    }
    if has(BuildingComponent::Placement)
        && let Some(placement) = c.placement.get(element)
    {
        let world = building.world_placement(element, cx)?;
        out.placement = Some(PlacementComponent {
            relative_to: placement.relative_to.as_ref().map(id),
            world: world.map(|t| t.0.iter().map(|row| row.to_vec()).collect()),
            unsupported: match &placement.local {
                Local::Unsupported(name) => Some(name.clone()),
                Local::Transform(_) => None,
            },
        });
    }
    if has(BuildingComponent::TypeOf) {
        out.type_of = c.r#type.get(element).map(|t| id(&t.target));
    }
    if has(BuildingComponent::Properties) {
        out.properties = c.properties.get(element).map(|found| {
            found
                .sets
                .iter()
                .map(|set| PropertySet {
                    name: set.name.clone(),
                    properties: set
                        .properties
                        .iter()
                        .map(|p| Property {
                            name: p.name.clone(),
                            value: p.value.as_ref().map(|v| value(model, v)),
                            unit: p.unit.as_ref().map(unit),
                        })
                        .collect(),
                })
                .collect()
        });
    }
    if has(BuildingComponent::Quantities) {
        out.quantities = c.quantities.get(element).map(|found| {
            found
                .sets
                .iter()
                .map(|set| QuantitySet {
                    name: set.name.clone(),
                    method: set.method.clone(),
                    quantities: set
                        .quantities
                        .iter()
                        .map(|q| Quantity {
                            name: q.name.clone(),
                            kind: match q.kind {
                                QuantityKind::Length => messages::QuantityKind::Length,
                                QuantityKind::Area => messages::QuantityKind::Area,
                                QuantityKind::Volume => messages::QuantityKind::Volume,
                                QuantityKind::Count => messages::QuantityKind::Count,
                                QuantityKind::Weight => messages::QuantityKind::Weight,
                                QuantityKind::Time => messages::QuantityKind::Time,
                            },
                            value: measured(&q.value),
                            formula: q.formula.clone(),
                        })
                        .collect(),
                })
                .collect()
        });
    }
    if has(BuildingComponent::Material) {
        out.material = c.material.get(element).map(|m| MaterialComponent {
            kind: match m.kind {
                MaterialKind::Single => messages::MaterialKind::Single,
                MaterialKind::Layers => messages::MaterialKind::Layers,
                MaterialKind::Profiles => messages::MaterialKind::Profiles,
                MaterialKind::Constituents => messages::MaterialKind::Constituents,
                MaterialKind::List => messages::MaterialKind::List,
            },
            parts: m
                .parts
                .iter()
                .map(|part| MaterialPart {
                    material: part.material.as_ref().map(id),
                    name: part.name.clone(),
                    thickness: part.thickness.as_ref().map(measured),
                })
                .collect(),
            direction: m.direction.clone(),
            offset: m.offset.as_ref().map(measured),
        });
    }
    if has(BuildingComponent::Opening) {
        out.opening = c.opening.get(element).map(|o| match o {
            Opening::Voids(target) => OpeningComponent {
                voids: Some(id(target)),
                fills: None,
            },
            Opening::Fills(target) => OpeningComponent {
                voids: None,
                fills: Some(id(target)),
            },
        });
    }
    if has(BuildingComponent::Group) {
        out.group = c.group.get(element).map(|g| model.ids(&g.groups));
    }
    if has(BuildingComponent::Connection) {
        out.connection = c.connection.get(element).map(|found| {
            found
                .links
                .iter()
                .map(|link| ConnectionLink {
                    kind: link.kind.clone(),
                    target: id(&link.target),
                })
                .collect()
        });
    }
    if has(BuildingComponent::Level) {
        out.level = c.level.get(element).map(|l| LevelComponent {
            name: l.name.clone(),
            kind: l.kind.clone(),
            height: measured(&l.height),
            datum: l.datum.clone(),
        });
    }
    if has(BuildingComponent::Zone) {
        out.zone = c.zone.get(element).map(|z| ZoneComponent {
            kind: z.kind.clone(),
            members: model.ids(&z.members),
        });
    }
    if has(BuildingComponent::Style) {
        out.style = c.style.get(element).map(|s| StyleComponent {
            surface_color: s.surface_color.map(|rgb| rgb.to_vec()),
            transparency: s.transparency,
            curve_color: s.curve_color.map(|rgb| rgb.to_vec()),
            layers: s.layers.clone(),
        });
    }
    if let Some(relations) = relations {
        out.relations = Some(RelationsComponent {
            children: model.ids(relations.children(element)),
            referenced: model.ids(relations.referenced(element)),
            instances: model.ids(relations.instances(element)),
            openings: model.ids(relations.openings(element)),
            fillings: model.ids(relations.fillings(element)),
            members: model.ids(relations.members(element)),
        });
    }
    Ok(out)
}

fn unit(unit: &Unit) -> BuildingUnit {
    BuildingUnit {
        kind: unit.kind.clone(),
        prefix: unit.prefix.clone(),
        name: unit.name.clone(),
        scale: unit.scale,
    }
}

fn measured(value: &zumen_building::Measured) -> Measured {
    Measured {
        value: value.value,
        unit: value.unit.as_ref().map(unit),
    }
}

fn value(model: &Model, value: &TypedValue) -> BuildingValue {
    let mut out = literal(model, &value.value);
    out.type_name = value.type_name.clone();
    out
}

fn literal(model: &Model, literal: &Literal) -> BuildingValue {
    let mut out = BuildingValue {
        type_name: None,
        kind: BuildingValueKind::Real,
        number: None,
        text: None,
        boolean: None,
        element: None,
        items: None,
    };
    match literal {
        Literal::Real(number) => out.number = Some(*number),
        Literal::Integer(number) => {
            out.kind = BuildingValueKind::Integer;
            // 2^53 − 1 を超える整数は、数にすると値が変わるので文字列にする
            if number.unsigned_abs() <= zumen_json::MAX_SAFE_INTEGER as u64 {
                out.number = Some(*number as f64);
            } else {
                out.text = Some(number.to_string());
            }
        }
        Literal::Text(text) => {
            out.kind = BuildingValueKind::Text;
            out.text = Some(text.clone());
        }
        Literal::Boolean(value) => {
            out.kind = BuildingValueKind::Boolean;
            out.boolean = Some(*value);
        }
        Literal::Logical(value) => {
            out.kind = BuildingValueKind::Logical;
            out.boolean = *value;
        }
        Literal::Enumeration(text) => {
            out.kind = BuildingValueKind::Enumeration;
            out.text = Some(text.clone());
        }
        Literal::Element(element) => {
            out.kind = BuildingValueKind::Element;
            out.element = Some(model.id(*element));
        }
        Literal::List(items) => {
            out.kind = BuildingValueKind::List;
            out.items = Some(
                items
                    .iter()
                    .map(|item| self::literal(model, item))
                    .collect(),
            );
        }
    }
    out
}

/// IDS で照合する。`express` を渡せば、IFC の属性の全体を EXPRESS の名前で照らす
pub(crate) fn check(
    model: &Model,
    ids: &[u8],
    express: Option<&[u8]>,
    cx: &mut Cx<'_>,
) -> Result<(Ids, Report), Failed> {
    let ids = Ids::read(ids, cx).map_err(|_| Failed)?;
    let schema = match express {
        Some(bytes) => Some(zumen_ifc::Express::read(bytes, cx).map_err(|_| Failed)?),
        None => None,
    };
    // プラグインの文書は、建物モデルだけで照合する
    let report = match &model.file {
        Some(file) => {
            let lookup = zumen_ifc::IdsLookup::new(file, schema.as_ref());
            zumen_ids::check(&model.building, &ids, &lookup, cx)
        }
        None => zumen_ids::check(&model.building, &ids, &model.building, cx),
    }
    .map_err(|_| Failed)?;
    Ok((ids, report))
}

/// 照合の指摘を、API の形にする。重さは仕様の重さ
pub(crate) fn finding(finding: &zumen_ids::Finding) -> IdsFinding {
    let mut diagnostic = message_of(&finding.diagnostic);
    diagnostic.severity = match finding.level {
        zumen_base::Severity::Info => messages::Severity::Info,
        zumen_base::Severity::Warning => messages::Severity::Warning,
        zumen_base::Severity::Error => messages::Severity::Error,
    };
    IdsFinding {
        specification: finding.specification.clone(),
        diagnostic,
    }
}

/// IDS で照合し、指摘の一部を返す
pub(crate) fn ids_check(
    ids: &Ids,
    report: &Report,
    offset: Option<u64>,
    limit: Option<u64>,
) -> IdsCheckResult {
    let total = report.findings.len() as u64;
    let start = offset.unwrap_or(0);
    let end = start.saturating_add(limit.unwrap_or(LIMIT));
    IdsCheckResult {
        title: ids.title().map(str::to_string),
        specifications: report
            .specifications
            .iter()
            .map(|s| IdsSpecification {
                name: s.name.clone(),
                identifier: s.identifier.clone(),
                status: match s.status {
                    zumen_ids::Status::Passed => IdsStatus::Passed,
                    zumen_ids::Status::Failed => IdsStatus::Failed,
                    zumen_ids::Status::Unknown => IdsStatus::Unknown,
                },
                applicable: s.applicable,
                failed: s.failed,
                unsupported: s.unsupported,
            })
            .collect(),
        findings: report
            .findings
            .iter()
            .skip(usize::try_from(start).unwrap_or(usize::MAX))
            .take(usize::try_from(end - start).unwrap_or(usize::MAX))
            .map(finding)
            .collect(),
        total,
        next_offset: (end < total).then_some(end),
    }
}
