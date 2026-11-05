#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let options = zumen_render::Options {
        width: 320,
        height: 240,
        region: [-100.0, -100.0, 100.0, 100.0],
        dpi: 96.0,
        background: [255; 4],
        foreground: [0, 0, 0, 255],
    };
    let previous = std::cell::RefCell::new(None);
    zumen_sdk::testkit::survives_with(data, |src, cx| {
        let result = zumen_render::svg(src, &options, cx);
        if let Some(first) = previous.borrow().as_ref() {
            assert_eq!(first, &result);
        }
        let status = result.as_ref().map(|_| ()).map_err(|error| match error {
            zumen_render::Error::Stopped(stop) => zumen_sdk::Error::Stopped(*stop),
            _ => zumen_sdk::Error::Rejected,
        });
        *previous.borrow_mut() = Some(result);
        status
    })
    .unwrap();
});
