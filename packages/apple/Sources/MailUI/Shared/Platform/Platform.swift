import SwiftUI

#if os(macOS)
import AppKit

typealias PlatformColor = NSColor
typealias PlatformFont = NSFont
typealias PlatformImage = NSImage
#else
import UIKit

typealias PlatformColor = UIColor
typealias PlatformFont = UIFont
typealias PlatformImage = UIImage
#endif

/// What the Mac and iOS do differently, behind one name each.
enum Platform {
    #if os(macOS)
    /// How much larger than on the Mac text and the controls around it are.
    static let scale: CGFloat = 1
    /// Whether a selected tab's or row's text is drawn in the stronger weight.
    static let strongSelection = false
    /// What the device is called in a sentence: "leaves this Mac".
    static let device = "Mac"
    #else
    static let scale: CGFloat = 1.15
    static let strongSelection = true
    @MainActor static var device: String { UIDevice.current.userInterfaceIdiom == .pad ? "iPad" : "iPhone" }
    #endif

    static func open(_ url: URL) {
        #if os(macOS)
        NSWorkspace.shared.open(url)
        #else
        UIApplication.shared.open(url)
        #endif
    }

    /// The app's version, as its bundle says.
    static var version: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
    }

    /// Where the core keeps its database.
    static let dataFolder: URL = {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let folder = base.appendingPathComponent(Bundle.main.bundleIdentifier ?? "com.yekta.mail", isDirectory: true)
        #if os(macOS)
        adoptOldMacInstall(into: folder, base: base)
        #endif
        return folder
    }()

    #if os(macOS)
    /// The Mac app was `com.yekta.mail.mac` before it shared the iOS app's App ID: its mail,
    /// session and settings move over, so nobody is signed out by the change.
    private static func adoptOldMacInstall(into folder: URL, base: URL) {
        let old = "com.yekta.mail.mac"
        let oldFolder = base.appendingPathComponent(old, isDirectory: true)
        guard Bundle.main.bundleIdentifier != old,
              !FileManager.default.fileExists(atPath: folder.path),
              FileManager.default.fileExists(atPath: oldFolder.path)
        else { return }
        try? FileManager.default.moveItem(at: oldFolder, to: folder)
        guard let settings = UserDefaults.standard.persistentDomain(forName: old) else { return }
        for (key, value) in settings where UserDefaults.standard.object(forKey: key) == nil {
            UserDefaults.standard.set(value, forKey: key)
        }
    }
    #endif

    /// Avenir, which both systems ship, at an exact size. Its weights step from Medium straight
    /// to Heavy, so Medium is the semibold and Heavy the bold.
    static func face(_ size: CGFloat, _ weight: PlatformFont.Weight = .regular) -> PlatformFont {
        let name = switch weight {
        case .ultraLight, .thin: "Avenir-Light"
        case .light: "Avenir-Book"
        case .medium, .semibold: "Avenir-Medium"
        case .bold, .heavy: "Avenir-Heavy"
        case .black: "Avenir-Black"
        default: "Avenir-Roman"
        }
        guard let font = PlatformFont(name: name, size: size) else {
            return .systemFont(ofSize: size, weight: weight)
        }
        return font
    }

    /// Avenir at a Mac size, enlarged on iOS.
    static func font(_ size: CGFloat, _ weight: PlatformFont.Weight = .regular) -> PlatformFont {
        face(size * scale, weight)
    }
}

extension Font {
    /// Avenir at a Mac size, enlarged on iOS.
    static func ui(_ size: CGFloat, _ weight: PlatformFont.Weight = .regular) -> Font {
        Font(Platform.font(size, weight) as CTFont)
    }
}

extension Image {
    init(platformImage: PlatformImage) {
        #if os(macOS)
        self.init(nsImage: platformImage)
        #else
        self.init(uiImage: platformImage)
        #endif
    }
}
