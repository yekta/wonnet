import Foundation
import Observation
import QuickLookThumbnailing

/// Pictures of messages' attachments, drawn by Quick Look from the files the core keeps. Images
/// and PDFs small enough are fetched as their cards show; any other file once it was opened.
@Observable @MainActor
final class AttachmentPreviews {
    static let shared = AttachmentPreviews()

    /// The pictures drawn so far, by `<message>/<index>`.
    private(set) var previews: [String: CGImage] = [:]

    @ObservationIgnored var bridge: CoreBridge?
    @ObservationIgnored private var asked: Set<String> = []

    /// The largest file fetched only to draw its picture.
    private static let fetchedAhead: Int64 = 5_000_000
    /// The picture's size: a card's preview, on a screen of twice the pixels.
    nonisolated private static let side = CGSize(width: 240, height: 160)

    static func key(_ message: String, _ index: Int) -> String { "\(message)/\(index)" }

    /// Draws an image's or a PDF's picture, fetching the file ahead.
    func load(message: String, index: Int, attachment: Attachment) async {
        let fetches = attachment.mime.hasPrefix("image/") || attachment.mime == "application/pdf"
        guard fetches, attachment.size <= Self.fetchedAhead else { return }
        await draw(message: message, index: index)
    }

    /// Draws the picture of a file already fetched, as one is once opened.
    func load(message: String, index: Int, file: URL) async {
        let key = Self.key(message, index)
        guard previews[key] == nil, let image = await Self.thumbnail(file) else { return }
        previews[key] = image
    }

    private func draw(message: String, index: Int) async {
        let key = Self.key(message, index)
        guard let bridge, !asked.contains(key) else { return }
        asked.insert(key)
        guard let reply = try? await bridge.call("open_attachment", ["message": message, "index": index], as: PathReply.self),
              let image = await Self.thumbnail(URL(fileURLWithPath: reply.path))
        else { return }
        previews[key] = image
    }

    nonisolated private static func thumbnail(_ file: URL) async -> CGImage? {
        let request = QLThumbnailGenerator.Request(fileAt: file, size: side, scale: 2, representationTypes: .thumbnail)
        return try? await QLThumbnailGenerator.shared.generateBestRepresentation(for: request).cgImage
    }
}
