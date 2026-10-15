import SwiftUI

/// A group of a settings page on the Mac: its name and a line about it, over a card of rows with
/// a hairline between them. `id` is what the settings' search scrolls to.
struct SettingsGroup<Content: View>: View {
    let id: String
    let title: String
    var detail: String?
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: Space.s + 2) {
            SectionHeading(title: title, detail: detail)
                .padding(.horizontal, Space.xxs)
            SettingsCard { content }
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
    }
}

/// One row of a `SettingsGroup`: what it is about on the left, its controls on the right. Every
/// row is at least as tall as the tallest control with the same room above and below. A control
/// at either end has as much room beside it as above and below it.
struct SettingsRow<Leading: View, Trailing: View>: View {
    @ViewBuilder let leading: Leading
    @ViewBuilder var trailing: Trailing
    @State private var leadingFace: ControlFace?
    @State private var trailingFace: ControlFace?

    var body: some View {
        HStack(spacing: Space.m) {
            HStack(spacing: Space.m) { leading }
                .onPreferenceChange(ControlFaceKey.self) { leadingFace = $0 }
                .padding(.leading, Self.inset(leadingFace))
            Spacer(minLength: Space.m)
            HStack(spacing: 0) { trailing }
                .onPreferenceChange(ControlFaceKey.self) { trailingFace = $0 }
                .padding(.trailing, Self.inset(trailingFace))
        }
        .padding(.vertical, Space.s)
        .frame(minHeight: Theme.settingsRowHeight * Platform.scale)
    }

    /// From the card's side to the end's hit area: a control's face as far in as it is from a
    /// row's top, words `Space.l`.
    private static func inset(_ face: ControlFace?) -> CGFloat {
        guard let face else { return Space.l }
        return min((Theme.settingsRowHeight * Platform.scale - face.height) / 2, Space.l) - face.inset
    }
}

extension SettingsRow where Trailing == EmptyView {
    init(@ViewBuilder leading: () -> Leading) {
        self.init(leading: leading, trailing: { EmptyView() })
    }
}

/// What a settings row is about: its name, a line under it, and a symbol or a dot before it.
/// `heading` sets its name in the heading's weight, for the row that heads a card.
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
