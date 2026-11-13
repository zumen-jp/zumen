#!/usr/bin/env python3
"""Pillow 13.0.0（libtiff・libpng）と tifffile 2026.11.12 による zumen-tiff の外部照合スクリプト（#877）。"""

from __future__ import annotations

import json
import pathlib
import sys


def check_versions() -> None:
    try:
        import numpy as np
        import PIL
        import tifffile
    except ImportError:
        sys.exit(77)

    if PIL.__version__ != "13.0.0":
        sys.stderr.write(f"unexpected Pillow version: {PIL.__version__}\n")
        sys.exit(1)
    if tifffile.__version__ != "2026.11.12":
        sys.stderr.write(f"unexpected tifffile version: {tifffile.__version__}\n")
        sys.exit(1)
    if np.__version__ != "2.5.4":
        sys.stderr.write(f"unexpected numpy version: {np.__version__}\n")
        sys.exit(1)

    print(f"pillow\t{PIL.__version__}\ttifffile\t{tifffile.__version__}")


def emit_cases(out_dir: pathlib.Path) -> None:
    import numpy as np
    from PIL import Image
    import tifffile

    out_dir.mkdir(parents=True, exist_ok=True)
    manifest: dict[str, object] = {"cases": []}

    def add_image_case(name: str, filename: str, expected_rgba: np.ndarray, page: int = 1, pages: int = 1) -> None:
        h, w, c = expected_rgba.shape
        assert c == 4
        raw_path = out_dir / f"{name}_expected.rgba"
        raw_path.write_bytes(expected_rgba.astype(np.uint8).tobytes())
        manifest["cases"].append(
            {
                "name": name,
                "file": filename,
                "expected_rgba": raw_path.name,
                "width": int(w),
                "height": int(h),
                "page": page,
                "pages": pages,
            }
        )

    # 1. Pillow（libtiff）: RGB 8-bit LZW
    yy, xx = np.mgrid[0:23, 0:37].astype(np.uint16)
    rgb = np.stack(
        [
            ((xx * 7 + yy * 3) & 0xFF),
            ((xx * 5 + yy * 11 + 40) & 0xFF),
            ((xx * 13 + yy * 17 + 90) & 0xFF),
        ],
        axis=-1,
    ).astype(np.uint8)
    Image.fromarray(rgb, mode="RGB").save(out_dir / "rgb_lzw.tif", compression="tiff_lzw")
    rgba1 = np.concatenate([rgb, np.full((23, 37, 1), 255, dtype=np.uint8)], axis=-1)
    add_image_case("rgb_lzw", "rgb_lzw.tif", rgba1)

    # 2. Pillow（libtiff）: グレースケール 8-bit PackBits
    yy, xx = np.mgrid[0:19, 0:41].astype(np.uint16)
    gray = (((xx // 4) * 37 + yy * 9) & 0xFF).astype(np.uint8)
    Image.fromarray(gray, mode="L").save(out_dir / "gray_packbits.tif", compression="packbits")
    rgba2 = np.stack([gray, gray, gray, np.full_like(gray, 255)], axis=-1)
    add_image_case("gray_packbits", "gray_packbits.tif", rgba2)

    # 3. Pillow（libtiff）: 2 値 1-bit CCITT Group 4（ITU-T T.6）
    yy, xx = np.mgrid[0:31, 0:67]
    bits = (((xx // 5) + (yy // 3)) % 2 == 0)
    bilevel_u8 = np.where(bits, 255, 0).astype(np.uint8)
    Image.fromarray(bilevel_u8, mode="L").convert("1", dither=Image.Dither.NONE).save(
        out_dir / "bilevel_g4.tif", compression="group4"
    )
    rgba3 = np.stack([bilevel_u8, bilevel_u8, bilevel_u8, np.full_like(bilevel_u8, 255)], axis=-1)
    add_image_case("bilevel_g4", "bilevel_g4.tif", rgba3)

    # 4. Pillow（libtiff）: RGBA 8-bit Adobe Deflate
    yy, xx = np.mgrid[0:17, 0:29].astype(np.uint16)
    rgba4 = np.stack(
        [
            ((xx * 9) & 0xFF),
            ((yy * 15) & 0xFF),
            (((xx + yy) * 6) & 0xFF),
            ((128 + xx * 4) & 0xFF),
        ],
        axis=-1,
    ).astype(np.uint8)
    # Pillow が書き出す ExtraSamples（1: 乗算済みアルファ、2: 非乗算アルファ）をタグから読み取り、TIFF 6.0 第 18 節に合わせて期待値を作る
    Image.fromarray(rgba4, mode="RGBA").save(out_dir / "rgba_deflate.tif", compression="tiff_adobe_deflate")
    with tifffile.TiffFile(out_dir / "rgba_deflate.tif") as tf:
        es = tf.pages[0].tags.get(338)
        es_val = es.value[0] if es is not None else 2
    if es_val == 1:
        # 乗算済みアルファ（ExtraSamples = 1）を TIFF 6.0 第 18 節に従い非乗算 RGBA8 に戻す
        exp4 = rgba4.copy()
        for r in range(17):
            for c in range(29):
                a = int(exp4[r, c, 3])
                if a == 0:
                    exp4[r, c, 0:3] = 0
                elif a < 255:
                    for ch in range(3):
                        exp4[r, c, ch] = min(255, (int(exp4[r, c, ch]) * 255 + a // 2) // a)
        add_image_case("rgba_deflate", "rgba_deflate.tif", exp4)
    else:
        add_image_case("rgba_deflate", "rgba_deflate.tif", rgba4)

    # 5. tifffile: タイル分割 8-bit RGB・zlib・Predictor=2（16x16 タイルで端数のある 35x26 画素）
    yy, xx = np.mgrid[0:26, 0:35].astype(np.uint16)
    rgb_tiled = np.stack(
        [
            ((xx * 11 + yy * 2) & 0xFF),
            ((xx * 3 + yy * 19) & 0xFF),
            ((200 - (xx * 5 & 0x7F)) & 0xFF),
        ],
        axis=-1,
    ).astype(np.uint8)
    tifffile.imwrite(
        out_dir / "rgb_tiled_pred2.tif",
        rgb_tiled,
        tile=(16, 16),
        compression="zlib",
        predictor=True,
        photometric="rgb",
    )
    rgba5 = np.concatenate([rgb_tiled, np.full((26, 35, 1), 255, dtype=np.uint8)], axis=-1)
    add_image_case("rgb_tiled_pred2", "rgb_tiled_pred2.tif", rgba5)

    # 6. tifffile: 16-bit RGB 面分離（PlanarConfiguration=2）・zlib・Predictor=2
    yy, xx = np.mgrid[0:15, 0:25].astype(np.uint32)
    rgb16 = np.stack(
        [
            ((xx * 257 * 8) & 0xFFFF),
            ((yy * 257 * 15) & 0xFFFF),
            (((xx + yy) * 257 * 5) & 0xFFFF),
        ],
        axis=0,  # planarconfig='separate' 用の (3, 15, 25) 配列
    ).astype(np.uint16)
    tifffile.imwrite(
        out_dir / "rgb16_planar.tif",
        rgb16,
        planarconfig="separate",
        compression="zlib",
        predictor=True,
        photometric="rgb",
    )
    rgb16_hwc = np.transpose(rgb16, (1, 2, 0))
    rgb8_from_16 = ((rgb16_hwc.astype(np.uint32) + 128) // 257).astype(np.uint8)
    rgba6 = np.concatenate([rgb8_from_16, np.full((15, 25, 1), 255, dtype=np.uint8)], axis=-1)
    add_image_case("rgb16_planar", "rgb16_planar.tif", rgba6)

    # 7. Pillow: 8-bit パレット・LZW
    yy, xx = np.mgrid[0:21, 0:33].astype(np.uint8)
    idx_img = ((xx + yy * 2) % 16).astype(np.uint8)
    pal_im = Image.fromarray(idx_img, mode="P")
    pal: list[int] = []
    for i in range(256):
        pal.extend([(i * 17) & 0xFF, (i * 43 + 20) & 0xFF, (255 - ((i * 29) & 0xFF))])
    pal_im.putpalette(pal)
    pal_im.save(out_dir / "palette8.tif", compression="tiff_lzw")
    rgba7 = np.asarray(pal_im.convert("RGBA"), dtype=np.uint8)
    add_image_case("palette8", "palette8.tif", rgba7)

    # 8. tifffile: 複数ページ TIFF（1 ページ目 RGB・2 ページ目グレースケール）
    mp1 = np.zeros((12, 20, 3), dtype=np.uint8)
    mp1[:, :, 0] = 210
    mp1[:, :, 1] = 90
    mp1[:, :, 2] = 45
    mp2 = np.full((18, 14), 173, dtype=np.uint8)
    with tifffile.TiffWriter(out_dir / "multipage.tif") as tw:
        tw.write(mp1, photometric="rgb")
        tw.write(mp2, photometric="minisblack")
    add_image_case(
        "multipage_p1",
        "multipage.tif",
        np.concatenate([mp1, np.full((12, 20, 1), 255, dtype=np.uint8)], axis=-1),
        page=1,
        pages=2,
    )
    add_image_case(
        "multipage_p2",
        "multipage.tif",
        np.stack([mp2, mp2, mp2, np.full_like(mp2, 255)], axis=-1),
        page=2,
        pages=2,
    )

    # 9. GeoTIFF オルソ RGB（EPSG:6677 JGD2011 平面直角 IX 系・PixelIsArea）
    ortho = np.full((12, 16, 3), [60, 140, 220], dtype=np.uint8)
    geokeys_ortho = [
        1, 1, 0, 3,
        1024, 0, 1, 1,
        1025, 0, 1, 1,  # 面積画素（PixelIsArea）
        3072, 0, 1, 6677,
    ]
    tifffile.imwrite(
        out_dir / "geotiff_ortho.tif",
        ortho,
        photometric="rgb",
        compression="zlib",
        extratags=[
            (33550, "d", 3, (0.5, 0.5, 0.0), True),
            (33922, "d", 6, (0.0, 0.0, 0.0, -12500.0, -34000.0, 0.0), True),
            (34735, "H", len(geokeys_ortho), geokeys_ortho, True),
        ],
    )
    manifest["ortho"] = {
        "file": "geotiff_ortho.tif",
        "epsg": 6677,
        "bl": [-12500.0, -34006.0],
        "tr": [-12492.0, -34000.0],
    }

    # 10. GeoTIFF Float32 DEM（EPSG:6677・標高基準 EPSG:6695・PixelIsPoint・GDAL_NODATA="-9999.0"）
    dem = np.zeros((10, 12), dtype=np.float32)
    expected_pts: list[list[float]] = []
    for r in range(10):
        for c in range(12):
            if (r == 0 and c == 0) or (r == 5 and c == 7):
                dem[r, c] = -9999.0
            elif r == 9 and c == 11:
                dem[r, c] = np.nan
            else:
                z = float(np.float32(25.0 + r * 1.25 + c * 0.5))
                dem[r, c] = z
                # 格子点画素（PixelIsPoint）: (col, row) の東座標は 100.0 + col * 2.0、北座標は 200.0 - row * 2.0
                expected_pts.append([200.0 - r * 2.0, 100.0 + c * 2.0, z])
    geokeys_dem = [
        1, 1, 0, 4,
        1024, 0, 1, 1,
        1025, 0, 1, 2,  # 格子点画素（PixelIsPoint）
        3072, 0, 1, 6677,
        4096, 0, 1, 6695,
    ]
    tifffile.imwrite(
        out_dir / "geotiff_dem.tif",
        dem,
        photometric="minisblack",
        compression="zlib",
        extratags=[
            (33550, "d", 3, (2.0, 2.0, 0.0), True),
            (33922, "d", 6, (0.0, 0.0, 0.0, 100.0, 200.0, 0.0), True),
            (34735, "H", len(geokeys_dem), geokeys_dem, True),
            (42113, "s", 0, "-9999.0", True),
        ],
    )
    manifest["dem"] = {
        "file": "geotiff_dem.tif",
        "epsg": 6677,
        "vertical_epsg": 6695,
        "points": expected_pts,
    }

    (out_dir / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")


def verify_png(png_path: pathlib.Path, expected_raw_path: pathlib.Path, width: int, height: int) -> None:
    import numpy as np
    from PIL import Image

    with Image.open(png_path) as im:
        assert im.format == "PNG", f"expected PNG, got {im.format}"
        assert im.mode == "RGBA", f"expected RGBA, got {im.mode}"
        assert im.size == (width, height), f"size mismatch: {im.size} != {(width, height)}"
        got = np.asarray(im, dtype=np.uint8)

    exp = np.frombuffer(expected_raw_path.read_bytes(), dtype=np.uint8).reshape((height, width, 4))
    if not np.array_equal(got, exp):
        diff = np.abs(got.astype(np.int16) - exp.astype(np.int16))
        max_diff = int(diff.max())
        idx = tuple(int(x) for x in np.unravel_index(np.argmax(diff), diff.shape))
        sys.stderr.write(
            f"pixel mismatch in {png_path.name}: max_diff={max_diff} at {idx}, got={got[idx[:2]].tolist()}, exp={exp[idx[:2]].tolist()}\n"
        )
        sys.exit(1)


def main() -> None:
    args = sys.argv[1:]
    if args == ["--check"]:
        check_versions()
        return
    if len(args) == 2 and args[0] == "--emit":
        emit_cases(pathlib.Path(args[1]))
        return
    if len(args) == 5 and args[0] == "--verify-png":
        verify_png(
            pathlib.Path(args[1]),
            pathlib.Path(args[2]),
            int(args[3]),
            int(args[4]),
        )
        return
    sys.stderr.write("usage: verify.py [--check | --emit <dir> | --verify-png <png> <expected.rgba> <w> <h>]\n")
    sys.exit(2)


if __name__ == "__main__":
    main()
