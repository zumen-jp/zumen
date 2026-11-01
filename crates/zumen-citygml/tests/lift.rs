//! 試験の中で作った CityGML（PLATEAU の形）を読み、地理の領域モデルに移す（決定 0233、#379）。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_citygml::CityGml;
use zumen_crs::{Crs, VerticalCrs};
use zumen_geo::{Form, GeoModel, Position, Value};
use zumen_geom3d::Shape;
use zumen_sdk::{Codec, Lift};

const BUILDING: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<core:CityModel xmlns:core="http://www.opengis.net/citygml/2.0"
  xmlns:bldg="http://www.opengis.net/citygml/building/2.0"
  xmlns:tran="http://www.opengis.net/citygml/transportation/2.0"
  xmlns:gen="http://www.opengis.net/citygml/generics/2.0"
  xmlns:gml="http://www.opengis.net/gml"
  xmlns:xlink="http://www.w3.org/1999/xlink"
  xmlns:uro="https://www.geospatial.jp/iur/uro/3.1">
  <gml:boundedBy>
    <gml:Envelope srsName="http://www.opengis.net/def/crs/EPSG/0/6697" srsDimension="3">
      <gml:lowerCorner>35.0 139.0 0</gml:lowerCorner>
      <gml:upperCorner>35.1 139.1 10</gml:upperCorner>
    </gml:Envelope>
  </gml:boundedBy>
  <core:cityObjectMember>
    <bldg:Building gml:id="bldg_1">
      <gml:name>庁舎</gml:name>
      <gen:stringAttribute name="建物ID"><gen:value>13101-bldg-1</gen:value></gen:stringAttribute>
      <gen:genericAttributeSet name="浸水">
        <gen:measureAttribute name="深さ"><gen:value uom="m">0.5</gen:value></gen:measureAttribute>
      </gen:genericAttributeSet>
      <bldg:class codeSpace="../../codelists/Building_class.xml">3001</bldg:class>
      <bldg:measuredHeight uom="m">9.5</bldg:measuredHeight>
      <bldg:storeysAboveGround>3</bldg:storeysAboveGround>
      <bldg:yearOfConstruction>1990</bldg:yearOfConstruction>
      <bldg:lod0RoofEdge>
        <gml:MultiSurface>
          <gml:surfaceMember>
            <gml:Polygon>
              <gml:exterior><gml:LinearRing><gml:posList>35.0 139.0 0 35.0 139.001 0 35.001 139.001 0 35.0 139.0 0</gml:posList></gml:LinearRing></gml:exterior>
            </gml:Polygon>
          </gml:surfaceMember>
        </gml:MultiSurface>
      </bldg:lod0RoofEdge>
      <bldg:lod2Solid>
        <gml:Solid>
          <gml:exterior>
            <gml:CompositeSurface>
              <gml:surfaceMember xlink:href="#wall_poly"/>
              <gml:surfaceMember xlink:href="#missing"/>
            </gml:CompositeSurface>
          </gml:exterior>
        </gml:Solid>
      </bldg:lod2Solid>
      <bldg:boundedBy>
        <bldg:WallSurface gml:id="wall_1">
          <bldg:lod2MultiSurface>
            <gml:MultiSurface>
              <gml:surfaceMember>
                <gml:Polygon gml:id="wall_poly">
                  <gml:exterior><gml:LinearRing><gml:posList>35.0 139.0 0 35.0 139.001 0 35.0 139.001 9.5 35.0 139.0 9.5 35.0 139.0 0</gml:posList></gml:LinearRing></gml:exterior>
                </gml:Polygon>
              </gml:surfaceMember>
            </gml:MultiSurface>
          </bldg:lod2MultiSurface>
        </bldg:WallSurface>
      </bldg:boundedBy>
      <uro:buildingDetailAttribute>
        <uro:BuildingDetailAttribute>
          <uro:surveyYear>2020</uro:surveyYear>
          <uro:buildingStructureType codeSpace="../../codelists/BuildingDetailAttribute_buildingStructureType.xml">601</uro:buildingStructureType>
        </uro:BuildingDetailAttribute>
      </uro:buildingDetailAttribute>
    </bldg:Building>
  </core:cityObjectMember>
  <core:cityObjectMember>
    <tran:Road>
      <tran:function codeSpace="../../codelists/Road_function.xml">1</tran:function>
      <tran:lod1MultiSurface>
        <gml:MultiSurface>
          <gml:surfaceMember>
            <gml:Polygon>
              <gml:exterior><gml:LinearRing><gml:posList>35.0 139.0 0 35.0 139.01 0 35.0001 139.01 0 35.0 139.0 0</gml:posList></gml:LinearRing></gml:exterior>
            </gml:Polygon>
          </gml:surfaceMember>
        </gml:MultiSurface>
      </tran:lod1MultiSurface>
      <tran:lod0Network>
        <gml:MultiCurve>
          <gml:curveMember><gml:LineString><gml:posList>35.0 139.0 0 35.0 139.01 0</gml:posList></gml:LineString></gml:curveMember>
        </gml:MultiCurve>
      </tran:lod0Network>
    </tran:Road>
  </core:cityObjectMember>
</core:CityModel>
"##;

#[cfg(test)]
fn read(text: &str) -> (GeoModel, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let model = {
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let file = CityGml::read(text.as_bytes(), &mut cx).unwrap();
        file.lift(&mut cx).unwrap()
    };
    (model, diag.iter().map(|d| d.code()).collect())
}

#[test]
fn buildings_and_roads_are_moved_with_attributes_and_shapes() {
    assert!(
        CityGml::SIGNATURES
            .iter()
            .any(|signature| signature.matches(BUILDING.as_bytes()))
    );
    let (model, diag) = read(BUILDING);
    assert_eq!(diag, [codes::CITYGML_REFERENCE_UNSUPPORTED], "{diag:?}");
    assert_eq!(model.crs, Some(Crs::Geographic));
    assert_eq!(model.height, Some(VerticalCrs::Jgd2011));
    let kinds: Vec<_> = model.features.iter().map(|f| f.kind.as_str()).collect();
    assert_eq!(kinds, ["bldg:Building", "bldg:WallSurface", "tran:Road"]);
    let building = &model.features[0];
    assert_eq!(building.id, "bldg_1");
    assert_eq!(building.name.as_deref(), Some("庁舎"));
    assert_eq!(building.code.as_deref(), Some("3001"));
    let value = |name: &str| {
        building
            .attributes
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.clone())
    };
    assert_eq!(
        value("gen:建物ID"),
        Some(Value::Text("13101-bldg-1".into()))
    );
    assert_eq!(
        value("gen:浸水/深さ"),
        Some(Value::Measure {
            value: 0.5,
            unit: Some("m".into())
        })
    );
    assert_eq!(
        value("bldg:measuredHeight"),
        Some(Value::Measure {
            value: 9.5,
            unit: Some("m".into())
        })
    );
    assert_eq!(value("bldg:storeysAboveGround"), Some(Value::Integer(3)));
    assert_eq!(
        value("uro:buildingDetailAttribute/uro:surveyYear"),
        Some(Value::Integer(2020))
    );
    assert!(matches!(
        value("uro:buildingDetailAttribute/uro:buildingStructureType"),
        Some(Value::Code { value, .. }) if value == "601"
    ));
    // LOD0 の屋根の縁と、壁の面を引いた LOD2 の立体
    let lods: Vec<_> = building.geometries.iter().map(|g| g.lod).collect();
    assert_eq!(lods, [Some(0), Some(2)]);
    let Form::Solid(Shape::FaceSet { faces, closed }) = &building.geometries[1].form else {
        panic!("{:?}", building.geometries[1]);
    };
    assert!(closed);
    assert_eq!(faces.len(), 1);
    // 閉じる点は除き、地理の空間の点は（緯度・経度・標高）
    assert_eq!(faces[0].outer.len(), 4);
    assert_eq!(faces[0].outer[2], [35.0, 139.001, 9.5]);
    // 壁は建築物の子の地物
    let wall = &model.features[1];
    assert_eq!((wall.id.as_str(), wall.parent), ("wall_1", Some(0)));
    // gml:id のない道路は、型と順の番号の ID
    let road = &model.features[2];
    assert_eq!(road.id, "tran:Road/1");
    assert!(matches!(
        road.attributes.as_slice(),
        [a] if a.name == "tran:function"
    ));
    let Some(Form::Line(line)) = road.geometries.iter().find_map(|g| match &g.form {
        line @ Form::Line(_) => Some(line.clone()),
        _ => None,
    }) else {
        panic!("{:?}", road.geometries);
    };
    assert_eq!(line.len(), 2);
    assert!(matches!(line[1].position, Position::Geo(p) if p.longitude == 139.01));
    assert_eq!(line[1].height, Some(0.0));
}

#[test]
fn without_a_known_crs_no_shapes_are_moved() {
    let text = BUILDING.replace("http://www.opengis.net/def/crs/EPSG/0/6697", "EPSG:4326");
    let (model, diag) = read(&text);
    assert_eq!(model.crs, None);
    assert!(diag.contains(&codes::CITYGML_CRS_UNSUPPORTED), "{diag:?}");
    assert_eq!(model.features.len(), 3);
    assert!(model.features.iter().all(|f| f.geometries.is_empty()));
    // 平面直角座標系の点は（easting・northing・標高）の順
    let text = BUILDING.replace("http://www.opengis.net/def/crs/EPSG/0/6697", "EPSG:6677");
    let (model, _) = read(&text);
    let Form::Solid(Shape::FaceSet { faces, .. }) = &model.features[0].geometries[1].form else {
        panic!();
    };
    assert_eq!(faces[0].outer[2], [139.001, 35.0, 9.5]);
}
