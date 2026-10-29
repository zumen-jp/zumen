//! 変更を仮に当てるための、予算と中断を確かめる複製。

use zumen_base::{Cx, Kind};
use zumen_sdk::Error;

pub(crate) fn check(cx: &mut Cx<'_>) -> Result<(), Error> {
    cx.check(Kind::Work, 1).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })
}

pub(crate) fn bytes(source: &[u8], cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
    let mut result = Vec::new();
    for chunk in source.chunks(1024) {
        check(cx)?;
        result.extend_from_slice(chunk);
    }
    Ok(result)
}
