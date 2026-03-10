// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "MosaicMacApp",
    platforms: [
        .macOS(.v14),
    ],
    products: [
        .library(name: "Domain", targets: ["Domain"]),
        .library(name: "Infrastructure", targets: ["Infrastructure"]),
        .library(name: "Features", targets: ["Features"]),
        .library(name: "UI", targets: ["UI"]),
        .executable(name: "MosaicMacApp", targets: ["MosaicMacApp"]),
    ],
    targets: [
        .target(name: "Domain"),
        .target(
            name: "Infrastructure",
            dependencies: ["Domain"]
        ),
        .target(
            name: "Features",
            dependencies: ["Domain", "Infrastructure"]
        ),
        .target(
            name: "UI",
            dependencies: ["Domain", "Features"]
        ),
        .executableTarget(
            name: "MosaicMacApp",
            dependencies: ["Domain", "Infrastructure", "Features", "UI"]
        ),
        .testTarget(
            name: "InfrastructureTests",
            dependencies: ["Domain", "Infrastructure"]
        ),
        .testTarget(
            name: "FeaturesTests",
            dependencies: ["Domain", "Infrastructure", "Features"]
        ),
        .testTarget(
            name: "UITests",
            dependencies: ["Domain", "Infrastructure", "Features", "UI"]
        ),
    ]
)
