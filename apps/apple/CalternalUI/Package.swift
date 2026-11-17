// swift-tools-version: 6.0
import PackageDescription

// Calternal's native design system: tokens generated from packages/ui/tokens/tokens.json, surfaces, the shell skeleton and the
// generated-UI renderer. No third-party dependencies. The token generator is a separate tool target and is not linked into the library.
let package = Package(
    name: "CalternalUI",
    platforms: [.macOS("26.0"), .iOS("27.0")],
    products: [.library(name: "CalternalUI", targets: ["CalternalUI"])],
    targets: [
        .target(name: "CalternalUI"),
        .target(name: "TokenGenerator"),
        .executableTarget(name: "generate-tokens", dependencies: ["TokenGenerator"]),
        .testTarget(name: "CalternalUITests", dependencies: ["CalternalUI", "TokenGenerator"]),
    ],
    swiftLanguageModes: [.v5]
)
