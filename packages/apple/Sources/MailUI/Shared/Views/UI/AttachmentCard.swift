import SwiftUI

/// A message's file as a card, as Gmail shows them under a message: a picture of it over its
/// name, its kind and its size. Until the picture is drawn, or for a file that has none, the
/// icon of its kind. On the card that fills the page it rests on `Theme.cardAccent`.
struct AttachmentCard: View {
    let name: String
    let mime: String
    let size: Int64
    var preview: CGImage?
    var pending = false
    let action: () -> Void

    var body: some View {
        Button(action: action) { Face(name: name, kind: kind, detail: detail, preview: preview, pending: pending) }
            .buttonStyle(.press)
            .help("Open \(name)")
    }

    private var detail: String {
        let size = ByteCountFormatter.string(fromByteCount: size, countStyle: .file)
        let type = (name as NSString).pathExtension.uppercased()
        return type.isEmpty || type.count > 5 ? size : "\(type) · \(size)"
    }

    private var kind: Symbol {
        let mime = mime.lowercased()
        let has = { (words: [String]) in words.contains { mime.contains($0) } }
        if mime.hasPrefix("image/") { return .fileImage }
        if mime.hasPrefix("video/") { return .fileVideo }
        if mime.hasPrefix("audio/") { return .fileAudio }
        if has(["spreadsheet", "excel", "csv", "numbers"]) { return .fileSpreadsheet }
        if has(["presentation", "powerpoint", "keynote"]) { return .presentation }
        if has(["zip", "compressed", "tar", "rar", "7z", "gzip"]) { return .fileArchive }
        if has(["pdf", "text", "word", "document", "rtf", "pages"]) { return .fileText }
        return .file
    }

    private struct Face: View {
        let name: String
        let kind: Symbol
        let detail: String
        let preview: CGImage?
        let pending: Bool
        @Environment(\.hovered) private var hovered
        @Environment(\.onCard) private var onCard

        var body: some View {
            VStack(alignment: .leading, spacing: 0) {
                picture
                    .frame(maxWidth: .infinity)
                    .frame(height: Theme.attachmentPreviewHeight * Platform.scale)
                    .background(previewFill.color)
                    .clipped()
                Rule(color: border)
                HStack(spacing: Space.xs) {
                    ButtonIcon(symbol: kind, pending: pending, size: 14, tint: Tokens.mutedForeground.color)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(name).textStyle(.caption, color: Tokens.foreground.color).lineLimit(1).truncationMode(.middle)
                        Text(detail).textStyle(.footnote).lineLimit(1)
                    }
                }
                .padding(.horizontal, Space.s)
                .padding(.vertical, Space.xs)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(footerFill.color)
            }
            .clipShape(RoundedRectangle(cornerRadius: Radius.md))
            .overlay(RoundedRectangle(cornerRadius: Radius.md).strokeBorder(border.color, lineWidth: Theme.hairline))
            .contentShape(RoundedRectangle(cornerRadius: Radius.md))
        }

        @ViewBuilder private var picture: some View {
            if let preview {
                Image(decorative: preview, scale: 2)
                    .resizable()
                    .scaledToFill()
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                    .clipped()
            } else {
                Image(kind, size: 28).foregroundStyle(Tokens.mutedStrongerForeground.color)
            }
        }

        private var border: ThemeColor { onCard ? Theme.cardRule : Tokens.border }

        private var previewFill: ThemeColor {
            if onCard { return hovered ? Theme.cardAccentStronger : Theme.cardAccent }
            return hovered ? Tokens.backgroundSecondaryAccent : Tokens.backgroundSecondary
        }

        private var footerFill: ThemeColor {
            if onCard { return hovered ? Theme.cardAccent : Theme.cardFill }
            return hovered ? Tokens.backgroundAccentLarger : Tokens.card
        }
    }
}
