// swift-tools-version:5.10
import PackageDescription

// What the Mac app and the iOS app share: the bridge to the Rust core, the state, and the views.
// The apps link the core's static library themselves. `MailPush` opens what pushes carry, for the
// app and the iOS app's notification extension.
let package = Package(
    name: "MailUI",
    platforms: [.macOS(.v14), .iOS("18.0")],
    products: [
        .library(name: "MailUI", targets: ["MailUI"]),
        .library(name: "MailPush", targets: ["MailPush"]),
    ],
    targets: [
        .target(name: "CMailCore", path: "Sources/CMailCore"),
        .target(name: "MailPush", path: "Sources/MailPush"),
        .target(name: "MailUI", dependencies: ["CMailCore", "MailPush"], path: "Sources/MailUI", resources: [.copy("Fonts")]),
    ],
    swiftLanguageVersions: [.v5]
)
