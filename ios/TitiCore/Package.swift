// swift-tools-version: 6.0
// TitiCore: Swift façade over the Rust core (uniffi Swift bindings + xcframework)
// plus transports written in Swift. The xcframework is produced on the macOS
// runner by scripts/ios-build-core.sh; on Windows only TitiTransport compiles
// (framing/test-vector logic) so it can be unit-tested with the Swift toolchain.
import PackageDescription
import Foundation

// The Rust xcframework exists only after scripts/ios-build-core.sh (macOS).
// Elsewhere (Windows/Linux CI) only TitiFrame + its tests are built.
let hasCore = FileManager.default.fileExists(atPath: URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("Artifacts/titi_ffiFFI.xcframework/Info.plist").path)

var products: [Product] = [.library(name: "TitiFrame", targets: ["TitiFrame"])]
var targets: [Target] = [
    .target(name: "TitiFrame", path: "Sources/TitiFrame"),
    .testTarget(name: "TitiFrameTests", dependencies: ["TitiFrame"], path: "Tests/TitiFrameTests", resources: [.copy("vectors")]),
]
if hasCore {
    products.append(.library(name: "TitiCore", targets: ["TitiCore"]))
    targets += [
        .target(name: "TitiCore", dependencies: ["TitiFrame", "titi_ffiFFI"], path: "Sources/TitiCore"),
        .binaryTarget(name: "titi_ffiFFI", path: "Artifacts/titi_ffiFFI.xcframework"),
    ]
}

let package = Package(
    name: "TitiCore",
    platforms: [.iOS(.v17), .macOS(.v14)],
    products: products,
    targets: targets
)
