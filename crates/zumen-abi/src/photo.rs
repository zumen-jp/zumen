//! `photo` の op（#387）。工事写真の Exif と XMP を読み、境界の形にする。

use zumen_base::Cx;

use crate::messages::{PhotoPosition, PhotoProperty, PhotoResult};
use crate::session::{Failed, stopped};

pub(crate) fn photo(bytes: &[u8], cx: &mut Cx<'_>) -> Result<PhotoResult, Failed> {
    let photo = zumen_photo::read(bytes, cx).map_err(|error| match error {
        zumen_sdk::Error::Stopped(stop) => stopped(cx, stop),
        // ほかの理由は、読む処理が診断として足してある
        _ => Failed,
    })?;
    Ok(PhotoResult {
        width: photo.width.map(u64::from),
        height: photo.height.map(u64::from),
        orientation: photo.orientation.map(u64::from),
        taken: photo.taken,
        make: photo.make,
        model: photo.model,
        position: photo.position.map(|point| PhotoPosition {
            latitude: point.latitude,
            longitude: point.longitude,
            altitude: photo.altitude,
        }),
        direction: photo.direction,
        direction_reference: photo.direction_reference,
        datum: photo.datum,
        xmp: photo
            .xmp
            .into_iter()
            .map(|property| PhotoProperty {
                namespace: property.namespace,
                name: property.name,
                value: property.value,
            })
            .collect(),
    })
}
