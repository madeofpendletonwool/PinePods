// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "PlaybackSupport",
    targets: [
        .target(name: "PlaybackSupport"),
        .testTarget(name: "PlaybackSupportTests", dependencies: ["PlaybackSupport"]),
    ]
)
