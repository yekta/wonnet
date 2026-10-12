import Contacts
import CoreText
import Foundation
import ImageIO
import UniformTypeIdentifiers

#if os(macOS)
import AppKit
private typealias SystemFont = NSFont
#else
import UIKit
private typealias SystemFont = UIFont
#endif

/// A sender's avatar: their contact photo, the picture the server found (their Gravatar or their
/// company's logo), or their initials on a colour picked from their address. The app's `Avatar`
/// picks the same colour.
public enum SenderAvatar {
    /// Which of the account colours an address gets.
    public static func colorIndex(_ email: String, count: Int) -> Int {
        let sum = email.unicodeScalars.reduce(0) { ($0 &* 31 &+ Int($1.value)) & 0xffff }
        return sum % count
    }

    /// The avatar as image data, for a notification.
    public static func image(initials: String, email: String, avatar: Data?) -> Data? {
        photo(email) ?? avatar ?? drawn(initials: initials, email: email)
    }

    /// The picture at the address a push gives, or nil when the sender has none.
    public static func download(_ url: URL) async -> Data? {
        let request = URLRequest(url: url, timeoutInterval: 5)
        guard let (data, response) = try? await URLSession.shared.data(for: request),
              (response as? HTTPURLResponse)?.statusCode == 200
        else { return nil }
        return data
    }

    /// Only with access already given: a notification never asks for it.
    private static func photo(_ email: String) -> Data? {
        let status = CNContactStore.authorizationStatus(for: .contacts)
        #if os(iOS)
        guard status == .authorized || status == .limited else { return nil }
        #else
        guard status == .authorized else { return nil }
        #endif
        let keys = [CNContactThumbnailImageDataKey as CNKeyDescriptor]
        let predicate = CNContact.predicateForContacts(matchingEmailAddress: email)
        let contacts = try? CNContactStore().unifiedContacts(matching: predicate, keysToFetch: keys)
        return contacts?.lazy.compactMap(\.thumbnailImageData).first
    }

    /// The initials on their colour, in a circle, as a PNG.
    private static func drawn(initials: String, email: String) -> Data? {
        let side = 192
        guard let context = CGContext(
            data: nil, width: side, height: side, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return nil }
        let colors = AccountColors.light
        let bounds = CGRect(x: 0, y: 0, width: side, height: side)
        context.setFillColor(color(colors[colorIndex(email, count: colors.count)]))
        context.fillEllipse(in: bounds)

        let font = SystemFont.systemFont(ofSize: CGFloat(side) * 0.4, weight: .semibold) as CTFont
        let text = NSAttributedString(string: initials, attributes: [
            NSAttributedString.Key(kCTFontAttributeName as String): font,
            NSAttributedString.Key(kCTForegroundColorAttributeName as String): color(AccountColors.foreground),
        ])
        let line = CTLineCreateWithAttributedString(text)
        let glyphs = CTLineGetBoundsWithOptions(line, .useGlyphPathBounds)
        context.textPosition = CGPoint(x: bounds.midX - glyphs.midX, y: bounds.midY - glyphs.midY)
        CTLineDraw(line, context)

        guard let image = context.makeImage() else { return nil }
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else { return nil }
        return data as Data
    }

    private static func color(_ hex: UInt32) -> CGColor {
        CGColor(
            srgbRed: CGFloat((hex >> 16) & 0xff) / 255, green: CGFloat((hex >> 8) & 0xff) / 255,
            blue: CGFloat(hex & 0xff) / 255, alpha: 1
        )
    }
}
