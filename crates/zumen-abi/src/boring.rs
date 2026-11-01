//! `boring` の op（#390）。ボーリング交換用データを読み、境界の形にする。

use zumen_base::Cx;
use zumen_crs::{HeightDatum as Datum, Realization};
use zumen_geology::Datum as Geodetic;

use crate::messages::{
    BoringColor, BoringDatum, BoringHeight, BoringLayer, BoringObservation, BoringPenetration,
    BoringPosition, BoringResult, BoringWaterLevel, HeightDatum, HeightRealization,
};
use crate::session::{Failed, stopped};

pub(crate) fn boring(bytes: &[u8], cx: &mut Cx<'_>) -> Result<BoringResult, Failed> {
    let boring = zumen_geology::read(bytes, cx).map_err(|error| match error {
        zumen_sdk::Error::Stopped(stop) => stopped(cx, stop),
        // ほかの理由は、読む処理が診断として足してある
        _ => Failed,
    })?;
    Ok(BoringResult {
        standards: boring.standards,
        publication: boring.publication,
        project: boring.project,
        survey: boring.survey,
        name: boring.name,
        count: boring.count.map(u64::from),
        serial: boring.serial.map(u64::from),
        position: boring.position.map(|point| BoringPosition {
            latitude: point.latitude,
            longitude: point.longitude,
        }),
        datum: boring.datum.map(|datum| match datum {
            Geodetic::Tokyo => BoringDatum::Tokyo,
            Geodetic::Jgd2000 => BoringDatum::Jgd2000,
            Geodetic::Jgd2011 => BoringDatum::Jgd2011,
            Geodetic::Jgd2024 => BoringDatum::Jgd2024,
        }),
        address: boring.address,
        start: boring.start,
        end: boring.end,
        elevation: boring.elevation.map(|height| BoringHeight {
            value: height.value,
            datum: match height.datum {
                Datum::TokyoBay => HeightDatum::TokyoBay,
                Datum::Island => HeightDatum::Island,
                Datum::Ellipsoid => HeightDatum::Ellipsoid,
            },
            realization: match height.realization {
                Realization::Survey2011 => HeightRealization::Survey2011,
                Realization::Survey2024 => HeightRealization::Survey2024,
            },
        }),
        length: boring.length,
        angle: boring.angle,
        direction: boring.direction,
        layers: boring
            .layers
            .into_iter()
            .map(|layer| BoringLayer {
                bottom: layer.bottom,
                name: layer.name,
                symbol: layer.symbol,
            })
            .collect(),
        colors: boring
            .colors
            .into_iter()
            .map(|color| BoringColor {
                bottom: color.bottom,
                name: color.name,
            })
            .collect(),
        observations: boring
            .observations
            .into_iter()
            .map(|observation| BoringObservation {
                top: observation.top,
                bottom: observation.bottom,
                text: observation.text,
            })
            .collect(),
        penetrations: boring
            .penetrations
            .into_iter()
            .map(|test| BoringPenetration {
                depth: test.depth,
                blows: u64::from(test.blows),
                penetration: test.penetration,
            })
            .collect(),
        water_levels: boring
            .water_levels
            .into_iter()
            .map(|level| BoringWaterLevel {
                date: level.date,
                state: level.state,
                depth: level.depth,
                note: level.note,
            })
            .collect(),
    })
}
