//! 要素と文書のハッシュ（決定 0156・0207）
//!
//! 成分を決まった並びのバイト列にし、BLAKE3 の先頭の 128 ビットを取る。出所（インスタンスの番号・元の GlobalId）と
//! 要素自身の ID は入れず、ほかの要素への参照は、指す先の ID で入れる。実数はビットの並びで書き、-0 は 0 に、NaN は
//! 1 つの値にそろえる。並びを変えるときは、[`Hash::VERSION`] を上げる。

use core::fmt;

use zumen_base::{Cx, Kind, Stop};

use crate::{
    Application, Building, Class, Classification, ClassificationReference, Connection, Context,
    Document, ElementRef, Extension, ExtensionValue, Grid, GridSet, Group, Header, Id, Identity,
    Level, Link, Literal, Local, MapConversion, Material, MaterialKind, MaterialPart, Measured,
    Membership, Opening, Parent, ParentKind, Placement, Properties, Property, PropertySet,
    Quantities, Quantity, QuantityKind, QuantitySet, Representation, ShapeItem,
    ShapeRepresentation, Spatial, SpatialKind, Style, Transform, TypeOf, TypedValue, Unit, Zone,
};

/// 要素か文書のハッシュ。`v1:` と 32 桁の 16 進で書く
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash(pub [u8; 16]);

impl Hash {
    /// 並びの版
    pub const VERSION: u32 = 1;

    /// 要素のハッシュ。成分だけから作る。ない要素は `None`
    pub fn element(building: &Building, element: ElementRef) -> Option<Self> {
        building.element(element)?;
        let mut out = Canon::new(building, None);
        encode_element(building, element, &mut out);
        Some(out.finish())
    }

    /// 予算と中断の合図を確かめながら要素をハッシュする。1024 バイトごとに `work` を 1 つ数える
    pub fn element_checked(
        building: &Building,
        element: ElementRef,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Self>, Stop> {
        if building.element(element).is_none() {
            return Ok(None);
        }
        let mut check = || cx.check(Kind::Work, 1);
        let mut out = Canon::new(building, Some(&mut check));
        encode_element(building, element, &mut out);
        if let Some(stop) = out.stop.take() {
            return Err(stop);
        }
        Ok(Some(out.finish()))
    }

    /// 文書の値のハッシュ。要素の成分は入れない
    pub fn document_values(building: &Building) -> Self {
        let mut out = Canon::new(building, None);
        b"document".encode(&mut out);
        building.document.encode(&mut out);
        out.finish()
    }

    /// 文書のハッシュ。元のバイト列そのものから作る（決定 0156）
    pub fn document(bytes: &[u8]) -> Self {
        Self::digest(blake3::hash(bytes))
    }

    /// 予算と中断の合図を確かめながら文書をハッシュする。1024 バイトごとに `work` を 1 つ数える
    pub fn document_checked(bytes: &[u8], cx: &mut Cx<'_>) -> Result<Self, Stop> {
        let mut hasher = blake3::Hasher::new();
        for chunk in bytes.chunks(1024) {
            cx.check(Kind::Work, 1)?;
            hasher.update(chunk);
        }
        Ok(Self::digest(hasher.finalize()))
    }

    fn digest(digest: blake3::Hash) -> Self {
        let mut bytes = [0; 16];
        for (byte, value) in bytes.iter_mut().zip(digest.as_bytes()) {
            *byte = *value;
        }
        Self(bytes)
    }
}

/// 成分の種類の名前を前に付け、要素が持つ成分だけを決まった順に入れる。種類を足しても、それを持たない要素の
/// ハッシュは変わらない
fn encode_element(building: &Building, element: ElementRef, out: &mut Canon<'_>) {
    fn put<T: Encode>(name: &str, value: Option<&T>, out: &mut Canon<'_>) {
        if let Some(value) = value {
            name.encode(out);
            value.encode(out);
        }
    }
    b"element".encode(out);
    let c = &building.components;
    put("identity", c.identity.get(element), out);
    put("class", c.class.get(element), out);
    put("classification", c.classification.get(element), out);
    put("spatial", c.spatial.get(element), out);
    put("parent", c.parent.get(element), out);
    put("placement", c.placement.get(element), out);
    put("representation", c.representation.get(element), out);
    put("style", c.style.get(element), out);
    put("type", c.r#type.get(element), out);
    put("properties", c.properties.get(element), out);
    put("quantities", c.quantities.get(element), out);
    put("material", c.material.get(element), out);
    put("opening", c.opening.get(element), out);
    put("group", c.group.get(element), out);
    put("connection", c.connection.get(element), out);
    put("grid", c.grid.get(element), out);
    put("level", c.level.get(element), out);
    put("zone", c.zone.get(element), out);
    put("extension", c.extension.get(element), out);
    // 成分の終わり。種類の名前は空でないので、空の名前と重ならない
    "".encode(out);
}

impl core::str::FromStr for Hash {
    type Err = ();

    /// `v` と版（[`Hash::VERSION`]）と `:` に続く、32 桁の 16 進を読む。版が違うか、桁か文字が合わなければ `Err`
    fn from_str(text: &str) -> Result<Self, ()> {
        let digits = text
            .strip_prefix('v')
            .and_then(|rest| rest.strip_prefix(Self::VERSION.to_string().as_str()))
            .and_then(|rest| rest.strip_prefix(':'))
            .ok_or(())?;
        if digits.len() != 32 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(());
        }
        let mut bytes = [0; 16];
        for (byte, pair) in bytes.iter_mut().zip(digits.as_bytes().chunks(2)) {
            let pair = core::str::from_utf8(pair).map_err(|_| ())?;
            *byte = u8::from_str_radix(pair, 16).map_err(|_| ())?;
        }
        Ok(Self(bytes))
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}:", Self::VERSION)?;
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

/// 決まった並びのバイト列を受ける所。要素への参照を ID に直すために、建物モデルを持つ
pub struct Canon<'a> {
    hasher: blake3::Hasher,
    building: &'a Building,
    check: Option<&'a mut dyn FnMut() -> Result<(), Stop>>,
    stop: Option<Stop>,
    pending: usize,
}

impl<'a> Canon<'a> {
    fn new(building: &'a Building, check: Option<&'a mut dyn FnMut() -> Result<(), Stop>>) -> Self {
        let mut out = Self {
            hasher: blake3::Hasher::new(),
            building,
            check,
            stop: None,
            pending: 0,
        };
        Hash::VERSION.encode(&mut out);
        out
    }

    fn bytes(&mut self, bytes: &[u8]) {
        if self.stop.is_some() {
            return;
        }
        // 1024 バイトごとに予算と中断の合図を確かめる
        self.pending += bytes.len();
        while self.pending >= 1024 {
            self.pending -= 1024;
            if let Some(check) = &mut self.check
                && let Err(stop) = check()
            {
                self.stop = Some(stop);
                return;
            }
        }
        self.hasher.update(bytes);
    }

    fn tag(&mut self, tag: u8) {
        self.bytes(&[tag]);
    }

    fn finish(self) -> Hash {
        Hash::digest(self.hasher.finalize())
    }
}

/// 決まった並びのバイト列にできる値
pub trait Encode {
    /// 値のバイト列を書き足す
    fn encode(&self, out: &mut Canon<'_>);
}

impl Encode for bool {
    fn encode(&self, out: &mut Canon<'_>) {
        out.tag(u8::from(*self));
    }
}

macro_rules! integers {
    ($($ty:ty),*) => {$(
        impl Encode for $ty {
            fn encode(&self, out: &mut Canon<'_>) {
                out.bytes(&self.to_le_bytes());
            }
        }
    )*};
}

integers!(u8, u32, u64, i64, u128);

impl Encode for usize {
    fn encode(&self, out: &mut Canon<'_>) {
        (*self as u64).encode(out);
    }
}

impl Encode for f64 {
    fn encode(&self, out: &mut Canon<'_>) {
        let bits = if self.is_nan() {
            f64::NAN.to_bits()
        } else if *self == 0.0 {
            0
        } else {
            self.to_bits()
        };
        bits.encode(out);
    }
}

impl Encode for [u8] {
    fn encode(&self, out: &mut Canon<'_>) {
        self.len().encode(out);
        out.bytes(self);
    }
}

impl<const N: usize> Encode for [u8; N] {
    fn encode(&self, out: &mut Canon<'_>) {
        self.as_slice().encode(out);
    }
}

impl Encode for str {
    fn encode(&self, out: &mut Canon<'_>) {
        self.as_bytes().encode(out);
    }
}

impl Encode for String {
    fn encode(&self, out: &mut Canon<'_>) {
        self.as_str().encode(out);
    }
}

impl<T: Encode> Encode for Vec<T> {
    fn encode(&self, out: &mut Canon<'_>) {
        self.len().encode(out);
        for item in self {
            if out.stop.is_some() {
                break;
            }
            item.encode(out);
        }
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            None => out.tag(0),
            Some(value) => {
                out.tag(1);
                value.encode(out);
            }
        }
    }
}

impl<A: Encode, B: Encode> Encode for (A, B) {
    fn encode(&self, out: &mut Canon<'_>) {
        self.0.encode(out);
        self.1.encode(out);
    }
}

impl Encode for [f64; 2] {
    fn encode(&self, out: &mut Canon<'_>) {
        self.iter().for_each(|v| v.encode(out));
    }
}

impl Encode for [f64; 3] {
    fn encode(&self, out: &mut Canon<'_>) {
        self.iter().for_each(|v| v.encode(out));
    }
}

impl Encode for Id {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Id::Global(id) => {
                out.tag(0);
                u128::from_be_bytes(id.to_uuid()).encode(out);
            }
            Id::Instance(number) => {
                out.tag(1);
                number.encode(out);
            }
        }
    }
}

/// 要素への参照は、指す先の ID で入れる。ない要素なら、番号を別の印で入れる
impl Encode for ElementRef {
    fn encode(&self, out: &mut Canon<'_>) {
        match out.building.id(*self) {
            Some(id) => id.encode(out),
            None => {
                out.tag(2);
                self.0.encode(out);
            }
        }
    }
}

impl Encode for Transform {
    fn encode(&self, out: &mut Canon<'_>) {
        for row in &self.0 {
            row.iter().for_each(|v| v.encode(out));
        }
    }
}

/// 構造体のフィールドを、書いた順に入れる。分解で書くので、フィールドを足し忘れるとコンパイルできない
macro_rules! records {
    ($($ty:ident { $($field:ident),* })*) => {$(
        impl Encode for $ty {
            fn encode(&self, out: &mut Canon<'_>) {
                let $ty { $($field),* } = self;
                $($field.encode(out);)*
            }
        }
    )*};
}

records! {
    Unit { kind, prefix, name, scale }
    Measured { value, unit }
    Document { project, project_name, phase, units, contexts, true_north, map, application, header }
    Context { identifier, kind, dimension, precision }
    MapConversion { crs_name, eastings, northings, height, x_axis_abscissa, x_axis_ordinate, scale }
    Application { name, version }
    Header { description, name, time_stamp, author, organization, preprocessor, originating_system, authorization, schemas }
    Identity { name, description, tag, long_name }
    Class { name, predefined, user_defined, original }
    Classification { references }
    ClassificationReference { system, edition, code, name }
    Spatial { kind, elevation, ref_elevation }
    Membership { target, kind, order }
    Parent { of, referenced_in }
    Placement { relative_to, local }
    ShapeRepresentation { context, kind, items }
    Representation { representations }
    Style { surface_color, transparency, curve_color, layers }
    TypeOf { target }
    TypedValue { type_name, value }
    Property { name, value, unit }
    PropertySet { name, properties }
    Properties { sets }
    Quantity { name, kind, value, formula }
    QuantitySet { name, method, quantities }
    Quantities { sets }
    MaterialPart { material, name, thickness }
    Material { kind, parts, direction, offset }
    Group { groups }
    Link { kind, target }
    Connection { links }
    Level { name, kind, height, datum }
    Zone { kind, members }
    ExtensionValue { namespace, name, value }
    Extension { values }
}

impl Encode for Literal {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Literal::Real(v) => {
                out.tag(0);
                v.encode(out);
            }
            Literal::Integer(v) => {
                out.tag(1);
                v.encode(out);
            }
            Literal::Text(v) => {
                out.tag(2);
                v.encode(out);
            }
            Literal::Boolean(v) => {
                out.tag(3);
                v.encode(out);
            }
            Literal::Logical(v) => {
                out.tag(4);
                v.encode(out);
            }
            Literal::Enumeration(v) => {
                out.tag(5);
                v.encode(out);
            }
            Literal::Element(v) => {
                out.tag(6);
                v.encode(out);
            }
            Literal::List(v) => {
                out.tag(7);
                v.encode(out);
            }
        }
    }
}

/// 形の項目の木は、深くても積みを使わないように、行きがけの順に、種類と子の数で入れる
impl Encode for ShapeItem {
    fn encode(&self, out: &mut Canon<'_>) {
        let mut stack = vec![self];
        while let Some(item) = stack.pop() {
            if out.stop.is_some() {
                break;
            }
            item.kind.encode(out);
            item.children.len().encode(out);
            stack.extend(item.children.iter().rev());
        }
    }
}

impl Encode for SpatialKind {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            SpatialKind::Site => out.tag(0),
            SpatialKind::Building => out.tag(1),
            SpatialKind::Storey => out.tag(2),
            SpatialKind::Space => out.tag(3),
            SpatialKind::SpatialZone => out.tag(4),
            SpatialKind::Other(name) => {
                out.tag(5);
                name.encode(out);
            }
        }
    }
}

impl Encode for ParentKind {
    fn encode(&self, out: &mut Canon<'_>) {
        out.tag(match self {
            ParentKind::Aggregates => 0,
            ParentKind::Nests => 1,
            ParentKind::Contained => 2,
        });
    }
}

impl Encode for Local {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Local::Transform(transform) => {
                out.tag(0);
                transform.encode(out);
            }
            Local::Unsupported(kind) => {
                out.tag(1);
                kind.encode(out);
            }
        }
    }
}

impl Encode for QuantityKind {
    fn encode(&self, out: &mut Canon<'_>) {
        out.tag(match self {
            QuantityKind::Length => 0,
            QuantityKind::Area => 1,
            QuantityKind::Volume => 2,
            QuantityKind::Count => 3,
            QuantityKind::Weight => 4,
            QuantityKind::Time => 5,
        });
    }
}

impl Encode for MaterialKind {
    fn encode(&self, out: &mut Canon<'_>) {
        out.tag(match self {
            MaterialKind::Single => 0,
            MaterialKind::Layers => 1,
            MaterialKind::Profiles => 2,
            MaterialKind::Constituents => 3,
            MaterialKind::List => 4,
        });
    }
}

impl Encode for Opening {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Opening::Voids(host) => {
                out.tag(0);
                host.encode(out);
            }
            Opening::Fills(opening) => {
                out.tag(1);
                opening.encode(out);
            }
        }
    }
}

impl Encode for GridSet {
    fn encode(&self, out: &mut Canon<'_>) {
        out.tag(match self {
            GridSet::U => 0,
            GridSet::V => 1,
            GridSet::W => 2,
        });
    }
}

impl Encode for Grid {
    fn encode(&self, out: &mut Canon<'_>) {
        match self {
            Grid::Grid { axes } => {
                out.tag(0);
                axes.encode(out);
            }
            Grid::Axis {
                tag,
                same_sense,
                curve,
            } => {
                out.tag(1);
                tag.encode(out);
                same_sense.encode(out);
                curve.encode(out);
            }
        }
    }
}
