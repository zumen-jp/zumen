// swift-tools-version:5.9
// Swift の包み（決定 0225）。CZumen は、`build-xcframework.sh` が作るネイティブのライブラリ（静的）とヘッダー。
import PackageDescription

let package = Package(
    name: "Zumen",
    platforms: [.macOS(.v12), .iOS(.v15)],
    products: [
        .library(name: "Zumen", targets: ["Zumen"]),
    ],
    targets: [
        .binaryTarget(name: "CZumen", path: "build/CZumen.xcframework"),
        .target(name: "Zumen", dependencies: ["CZumen"]),
        .executableTarget(name: "ZumenConformance", dependencies: ["Zumen"]),
    ]
)
