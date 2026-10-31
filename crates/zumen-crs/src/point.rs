//! 空間ごとの点の型と、空間の間の変換（決定 0019 の「座標の空間」）。

/// 地図の点（m）。平面直角座標系の X が `northing`、Y が `easting` に当たる。`x`・`y` とは呼ばない（決定 0019）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapPoint {
    /// 北の向きの座標（m）
    pub northing: f64,
    /// 東の向きの座標（m）
    pub easting: f64,
}

/// 地理の点（度）。北と東を正にする
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeoPoint {
    /// 緯度（度）
    pub latitude: f64,
    /// 経度（度）
    pub longitude: f64,
}

/// プロジェクトの空間の点（文書の長さの単位）。Z が上（決定 0019）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectPoint {
    /// X
    pub x: f64,
    /// Y
    pub y: f64,
    /// Z（上）
    pub z: f64,
}

/// 地図の点と、地図の高さの基準からの高さ（m）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapPosition {
    /// 地図の点
    pub point: MapPoint,
    /// 高さ。基準と成果は、変換が名指す高さの座標系が決める
    pub height: f64,
}

/// 平面の 2 つの成分の並び。形式が書く順を、読み書きの所で型に直すために使う（決定 0019）
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Axes {
    /// 1 つ目が東（右）、2 つ目が北（上）。数学の向き
    #[default]
    EastNorth,
    /// 1 つ目が北、2 つ目が東。測量の向き（平面直角座標系の X・Y、SXF の測地座標系の部分図）
    NorthEast,
}

impl Axes {
    /// この並びの点を、東・北の順にする
    pub fn east_north(self, [a, b]: [f64; 2]) -> [f64; 2] {
        match self {
            Axes::EastNorth => [a, b],
            Axes::NorthEast => [b, a],
        }
    }

    /// [`Axes::east_north`] をアフィン変換 `(a x + b y + c, d x + e y + f)` の `[a, b, c, d, e, f]` で表す
    pub fn to_east_north(self) -> [f64; 6] {
        match self {
            Axes::EastNorth => [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            Axes::NorthEast => [0.0, 1.0, 0.0, 1.0, 0.0, 0.0],
        }
    }

    /// この並びの点を、地図の点にする。`meters` は 1 単位の m の数
    pub fn map_point(self, point: [f64; 2], meters: f64) -> MapPoint {
        let [easting, northing] = self.east_north(point);
        MapPoint {
            northing: northing * meters,
            easting: easting * meters,
        }
    }
}

/// プロジェクトの空間から地図への変換（IFC の `IfcMapConversion`）。
///
/// IFC 4.3 の `IfcMapConversion` の定義のとおり、3 つの軸に同じ尺度を掛け、Z 軸のまわりに反時計回りに回し、
/// `Eastings`・`Northings`・`OrthogonalHeight` だけ動かす。回す角は、地図の上のプロジェクトの X 軸の向き
/// （`XAxisAbscissa`・`XAxisOrdinate`）で決まる
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapConversion {
    /// プロジェクトの原点の、地図の東の座標（`Eastings`）
    pub eastings: f64,
    /// プロジェクトの原点の、地図の北の座標（`Northings`）
    pub northings: f64,
    /// プロジェクトの原点の高さ（`OrthogonalHeight`）
    pub orthogonal_height: f64,
    /// X 軸の向きの東の成分（`XAxisAbscissa`）
    pub x_axis_abscissa: f64,
    /// X 軸の向きの北の成分（`XAxisOrdinate`）
    pub x_axis_ordinate: f64,
    /// プロジェクトの単位から地図の単位への尺度（`Scale`。書かれていなければ 1）
    pub scale: f64,
}

impl MapConversion {
    /// X 軸の向きの単位ベクトル（東・北）。向きの長さが 0 か、値が有限でなければ `None`
    fn direction(&self) -> Option<[f64; 2]> {
        let [a, o] = [self.x_axis_abscissa, self.x_axis_ordinate];
        let length = (a * a + o * o).sqrt();
        (length.is_finite() && length > 0.0).then(|| [a / length, o / length])
    }

    /// 尺度が正の有限の値で、向きが決まり、動かす量が有限なら真
    fn valid(&self) -> bool {
        self.scale.is_finite()
            && self.scale > 0.0
            && self.direction().is_some()
            && [self.eastings, self.northings, self.orthogonal_height]
                .iter()
                .all(|v| v.is_finite())
    }

    /// プロジェクトの点を地図に写す。変換が使えないか、結果が有限でなければ `None`
    pub fn to_map(&self, point: ProjectPoint) -> Option<MapPosition> {
        if !self.valid() {
            return None;
        }
        let [cos, sin] = self.direction()?;
        let [x, y, z] = [point.x, point.y, point.z].map(|v| v * self.scale);
        let position = MapPosition {
            point: MapPoint {
                northing: x * sin + y * cos + self.northings,
                easting: x * cos - y * sin + self.eastings,
            },
            height: z + self.orthogonal_height,
        };
        [
            position.point.northing,
            position.point.easting,
            position.height,
        ]
        .iter()
        .all(|v| v.is_finite())
        .then_some(position)
    }

    /// 地図の点をプロジェクトに写す（[`MapConversion::to_map`] の逆）
    pub fn to_project(&self, position: MapPosition) -> Option<ProjectPoint> {
        if !self.valid() {
            return None;
        }
        let [cos, sin] = self.direction()?;
        let e = position.point.easting - self.eastings;
        let n = position.point.northing - self.northings;
        let point = ProjectPoint {
            x: (e * cos + n * sin) / self.scale,
            y: (n * cos - e * sin) / self.scale,
            z: (position.height - self.orthogonal_height) / self.scale,
        };
        [point.x, point.y, point.z]
            .iter()
            .all(|v| v.is_finite())
            .then_some(point)
    }
}
