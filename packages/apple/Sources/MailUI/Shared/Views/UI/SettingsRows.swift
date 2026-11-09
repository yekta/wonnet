import SwiftUI

/// A group of a settings page on the Mac: its name and a line about it, over a card of rows with
/// a hairline between them. `id` is what the settings' search scrolls to. A group of one control
/// that is a field of its own, as the MCP server's address, has it on the page without a `card`.
struct SettingsGroup<Content: View>: View {
    let id: String
    let title: String
    var detail: String?
    var card = true
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: Space.s + 2) {
            SectionHeading(title: title, detail: detail)
                .padding(.horizontal, Space.xxs)
            if card {
                SettingsCard { content }
            } else {
                content
            }
        }
        .id(id)
    }
}

/// The card of a settings group's rows, alone: for a page of a card each, as each account's.
struct SettingsCard<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        VStack(spacing: 0) { content }
            .background(RoundedRectangle(cornerRadius: Radius.md).fill(Tokens.card.color))
            .overlay(RoundedRectangle(cornerRadius: Radius.md).strokeBorder(Tokens.border.color, lineWidth: Theme.hairline))
            .clipShape(RoundedRectangle(cornerRadius: Radius.md))
            .environment(\.onFormCard, true)
    }
}

/// One row of a `SettingsGroup`: what it is about on the left, its controls on the right. Every
/// row is at least as tall as the tallest control with the same room above and below. In a row of
/// one line, a control at either end has as much room beside it as above and below it; in a row
/// of more, it stands `Space.l` from its side, as the words do from theirs.
struct SettingsRow<Leading: View, Trailing: View>: View {
    @ViewBuilder let leading: Leading
    @ViewBuilder var trailing: Trailing
    @State private var leadingFace: ControlFace?
    @State private var trailingFace: ControlFace?
    @State private var height = Theme.settingsRowHeight * Platform.scale

    var body: some View {
        HStack(spacing: Space.m) {
            HStack(spacing: Space.m) { leading }
                .onPreferenceChange(ControlFaceKey.self) { leadingFace = $0 }
                .padding(.leading, inset(leadingFace))
            Spacer(minLength: Space.m)
            HStack(spacing: 0) { trailing }
                .onPreferenceChange(ControlFaceKey.self) { trailingFace = $0 }
                .padding(.trailing, inset(trailingFace))
        }
        .padding(.vertical, Space.s)
        .frame(minHeight: Theme.settingsRowHeight * Platform.scale)
        .onGeometryChange(for: CGFloat.self, of: \.size.height) { height = $0 }
    }

    /// From the card's side to the end's hit area: in a row of one line a control's face as far
    /// in as it is from the row's top, otherwise and for words `Space.l`.
    private func inset(_ face: ControlFace?) -> CGFloat {
        guard let face else { return Space.l }
        let oneLine = height <= Theme.settingsRowHeight * Platform.scale + 1
        return (oneLine ? min((height - face.height) / 2, Space.l) : Space.l) - face.inset
    }
}

extension SettingsRow where Trailing == EmptyView {
    init(@ViewBuilder leading: () -> Leading) {
        self.init(leading: leading, trailing: { EmptyView() })
    }
}

/// A `SettingsRow` a `MovableList` moves inside a settings card, keeping the card's hairline
/// under it, over the row that adds one. One that `opens` something hovers in the card's accent;
/// the `first` rounds its fill into the card's corners, which don't clip the list's rows.
struct MovableSettingsRow<Leading: View, Trailing: View>: View {
    var opens = false
    var first = false
    @ViewBuilder let leading: Leading
    @ViewBuilder let trailing: Trailing
    @Environment(\.hovered) private var hovered

    var body: some View {
        SettingsRow { leading } trailing: { trailing }
            .background(fill.fill(hovered && opens ? Tokens.cardAccent.color : .clear))
            .rule(.bottom, color: Tokens.borderCard)
    }

    private var fill: UnevenRoundedRectangle {
        let corner = first ? Radius.md : 0
        return UnevenRoundedRectangle(topLeadingRadius: corner, topTrailingRadius: corner)
    }
}

/// What a settings row is about: its name, a line under it, and a symbol or a dot before it.
/// `heading` sets its name in the heading's weight, for the row that heads a card. With a line
/// under it, the row keeps `Space.m` above and below its words.
struct SettingsLabel: View {
    let title: String
    var detail: String?
    var symbol: Symbol?
    var dot: Color?
    var heading = false
    /// How many lines the detail is cut to, if it is: a snippet's text.
    var detailLines: Int?

    var body: some View {
        HStack(spacing: Space.s + 2) {
            if let dot {
                Circle().fill(dot).frame(width: heading ? 10 : 8, height: heading ? 10 : 8)
            } else if let symbol {
                Image(symbol, size: 14).foregroundStyle(Tokens.mutedForeground.color)
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(title).textStyle(heading ? .subheading : .body).lineLimit(1).truncationMode(.middle)
                if let detail {
                    Text(detail).textStyle(.caption).lineLimit(detailLines).fixedSize(horizontal: false, vertical: detailLines == nil)
                }
            }
        }
        .padding(.vertical, detail == nil ? 0 : Space.m - Space.s)
    }
}

/// The hairline between the rows of a `SettingsGroup`.
struct SettingsDivider: View {
    var body: some View {
        Rule(color: Tokens.borderCard)
    }
}

/// A row of a `SettingsGroup` that only says something: nothing set yet.
struct SettingsNote: View {
    let text: String

    var body: some View {
        SettingsRow {
            Text(text).textStyle(.label, color: Tokens.mutedForeground.color).fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// Keys to press, one cap each: ⌘K, or G then I.
struct KeyCaps: View {
    let caps: [String]

    var body: some View {
        HStack(spacing: Space.xxs) {
            ForEach(caps.indices, id: \.self) { index in
                Text(caps[index])
                    .textStyle(.footnoteStrong, color: Tokens.foreground.color)
                    .padding(.horizontal, Space.xs)
                    .frame(minWidth: 22, minHeight: 20)
                    .background(RoundedRectangle(cornerRadius: Radius.xs).fill(Tokens.backgroundSecondary.color))
                    .overlay(RoundedRectangle(cornerRadius: Radius.xs).strokeBorder(Tokens.border.color, lineWidth: Theme.hairline))
            }
        }
    }
}
