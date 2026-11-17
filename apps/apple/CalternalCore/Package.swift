// swift-tools-version: 6.0
import PackageDescription
import Foundation

// The generated bridge links the library built from the same checkout.
let rustLibrary = ProcessInfo.processInfo.environment["CALTERNAL_RUST_LIB_DIR"]
    ?? URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        .appendingPathComponent("../../../target/debug").standardized.path
let package = Package(
    name: "CalternalCore",
    platforms: [.macOS("26.0")],
    products: [.library(name: "CalternalCore", targets: ["CalternalCore"])],
    targets: [
        .target(name: "calternal_bindingsFFI", publicHeadersPath: "include"),
        .target(name: "CalternalCore", dependencies: ["calternal_bindingsFFI"],
                linkerSettings: [.linkedLibrary("calternal_bindings"),
                    .unsafeFlags(["-L", rustLibrary, "-Xlinker", "-rpath", "-Xlinker", rustLibrary])]),
        .target(name: "BridgeChecks", dependencies: ["CalternalCore"], path: "Tests/BridgeChecks"),
        .executableTarget(name: "CalternalCoreProbe", dependencies: ["BridgeChecks"]),
        .testTarget(name: "CalternalCoreTests", dependencies: ["BridgeChecks"])
    ],
    swiftLanguageModes: [.v5]
)
