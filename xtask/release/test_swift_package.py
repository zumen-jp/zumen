"""SwiftPM のチェックサム、XCFramework の対象、静的書庫の依存照合を確かめる。"""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import sbom
import swift_package as swift
from test_package import notice


class SwiftPackageTests(unittest.TestCase):
    def test_mirror_uses_tagged_zip_and_checksum(self):
        checksum = "a1" * 32
        manifest, declaration = swift.mirror_manifest(checksum)
        self.assertIn(f"/releases/download/v{swift.VERSION}/{swift.ZIP}", declaration)
        self.assertIn(f'checksum: "{checksum}"', declaration)
        self.assertIn(declaration, manifest)
        self.assertNotIn(swift.LOCAL_TARGET, manifest)
        self.assertNotIn("executableTarget", manifest)
        self.assertIn("platforms: [.macOS(.v14), .iOS(.v17)]", manifest)

    def test_framework_distinguishes_ios_device_and_simulator(self):
        rows = [{"LibraryIdentifier": name, "LibraryPath": swift.SWIFT_LIBRARY, "HeadersPath": "Headers",
                 "SupportedArchitectures": ["arm64"], "SupportedPlatform": name.split("-")[0]}
                for name in ("macos-arm64", "ios-arm64", "ios-arm64-simulator")]
        rows[-1]["SupportedPlatformVariant"] = "simulator"
        swift.validate_slices({"AvailableLibraries": rows})
        with self.assertRaisesRegex(ValueError, "対象"):
            swift.validate_slices({"AvailableLibraries": [*rows[:2], rows[1]]})
        del rows[-1]["SupportedPlatformVariant"]
        with self.assertRaisesRegex(ValueError, "プラットフォーム"):
            swift.validate_slices({"AvailableLibraries": rows})

    def test_static_sbom_covers_archive_and_reads_its_audit_member(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            metadata = root / "macos-arm64"
            info, _ = notice(metadata, "abi", "aarch64-apple-darwin")
            document = json.loads((metadata / sbom.DRAFT).read_bytes())
            for prop in document["metadata"]["properties"]:
                if prop["name"] == "zumen:artifact":
                    prop["value"] = swift.SWIFT_LIBRARY
            (metadata / sbom.DRAFT).write_bytes(sbom.encode(document))
            library = root / swift.SWIFT_LIBRARY
            library.write_bytes(b"synthetic static library")
            shipped = sbom.bind(metadata, info, swift.SWIFT_LIBRARY, library.read_bytes())
            members = swift.AUDIT_OBJECT.encode() + b"\n"
            with patch("swift_package.ar", side_effect=[members, b"synthetic object"]) as ar, \
                    patch("sbom.extracted_graph", return_value=sbom.document_graph(document)) as graph:
                swift.verify_static(library, metadata, shipped)
                self.assertEqual(ar.call_args.args, ("p", library, swift.AUDIT_OBJECT))
                graph.assert_called_once_with(b"synthetic object")
            with patch("swift_package.ar", return_value=members * 2), self.assertRaisesRegex(ValueError, "一意"):
                swift.verify_static(library, metadata, shipped)
            with patch("swift_package.ar", side_effect=[members, b"other object"]), \
                    patch("sbom.extracted_graph", return_value=("other root", {})), self.assertRaisesRegex(ValueError, "依存"):
                swift.verify_static(library, metadata, shipped)
            library.write_bytes(b"changed static library")
            with self.assertRaisesRegex(ValueError, "ハッシュ"):
                swift.verify_static(library, metadata, shipped)


if __name__ == "__main__":
    unittest.main()
