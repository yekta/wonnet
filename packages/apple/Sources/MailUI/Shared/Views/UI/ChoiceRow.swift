import SwiftUI

/// One choice of a short list: a suggestion, a time, a snippet, a sender. It starts with a
/// `symbol`, an account's `dot` or a person's `avatar`; `subtitle` is a smaller line under its
/// title, `detail` a word at its end. `checked` is the one in force, with a check at its end: the
/// check's place is kept in every row, so nothing moves when it shows. `highlighted` is the one
/// the arrow keys are on.
struct ChoiceRow: View {
    let title: String
    var subtitle: String?
    var detail: String?
    var symbol: Symbol?
    var dot: Color?
    var avatar: Address?
    var checked = false
    var highlighted = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Face(
                title: title, subtitle: subtitle, detail: detail, symbol: symbol, dot: dot, avatar: avatar, checked: checked,
                highlighted: highlighted
            )
        }
        .buttonStyle(.press)
    }

    private struct Face: View {
        let title: String
        let subtitle: String?
        let detail: String?
        let symbol: Symbol?
        let dot: Color?
        let avatar: Address?
        let checked: Bool
        let highlighted: Bool
        @Environment(\.hovered) private var hovered
        @Environment(\.onPopover) private var onPopover

        /// The width of what a row starts with: an avatar's, and the place a symbol or a dot stands in.
        static let lead: CGFloat = 24

        private var hasLead: Bool { avatar != nil || dot != nil || symbol != nil }

        var body: some View {
            HStack(spacing: Space.s + 2) {
                // Whatever a row starts with, its words start where the others' do.
                Group {
                    if let avatar {
                        Avatar(initials: avatar.initials, email: avatar.email, size: Self.lead)
                    } else if let dot {
                        Circle().fill(dot).frame(width: 8, height: 8)
                    } else if let symbol {
                        Image(symbol, size: 14).foregroundStyle(Tokens.mutedStrongerForeground.color)
                    }
                }
                .frame(width: hasLead ? Self.lead : nil)
                VStack(alignment: .leading, spacing: 1) {
                    Text(title).textStyle(.body).lineLimit(1)
                    if let subtitle {
                        Text(subtitle).textStyle(.footnote, color: Tokens.mutedForeground.color).lineLimit(1).truncationMode(.middle)
                    }
                }
                Spacer(minLength: Space.m)
                if let detail {
                    Text(detail).textStyle(.caption).lineLimit(1).truncationMode(.middle).monospacedDigit()
                }
                Image(.check, size: 14).foregroundStyle(Tokens.foreground.color).opacity(checked ? 1 : 0)
                    .accessibilityHidden(!checked)
            }
            .padding(.horizontal, Space.s + 2)
            .padding(.vertical, subtitle == nil ? 0 : Space.xxs)
            .frame(minHeight: Theme.rowHeight * Platform.scale)
            .background(RoundedRectangle(cornerRadius: Radius.sm).fill(fill))
            .contentShape(Rectangle())
        }

        /// On a popover, which is lighter than the page, the fills are the popover's own.
        private var fill: Color {
            if highlighted { return (onPopover ? Tokens.popoverAccentStronger : Tokens.backgroundAccentLargerStronger).color }
            if hovered { return (onPopover ? Tokens.popoverAccent : Tokens.backgroundAccentLarger).color }
            return .clear
        }
    }
}

/// Choices under a field, one highlighted, scrolled to as the keys move; `empty` when there
/// are none.
struct ChoiceList<Item: Identifiable, Row: View>: View {
    let items: [Item]
    let highlighted: Int
    var empty: String?
    @ViewBuilder let row: (Int, Item) -> Row

    var body: some View {
        ScrollViewReader { scroller in
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                        row(index, item).id(item.id)
                    }
                    if items.isEmpty, let empty {
                        Text(empty).textStyle(.label, color: Tokens.mutedStrongerForeground.color)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(Space.s + 2)
                    }
                }
                .padding(Space.s)
            }
            .onChange(of: highlighted) { _, index in
                guard items.indices.contains(index) else { return }
                scroller.scrollTo(items[index].id)
            }
        }
    }
}

extension Int {
    /// The index `step` away, kept inside a list of `count`.
    func moved(by step: Int, in count: Int) -> Int {
        Swift.max(0, Swift.min(self + step, count - 1))
    }
}

/// A card floating over the page, for suggestions and short menus. `inset` is the space to its
/// edges; none when the content keeps its own, as a combobox's field and list do.
struct PopupCard<Content: View>: View {
    var inset: CGFloat = Space.xs
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 0) { content }
            .padding(inset)
            .clipShape(RoundedRectangle(cornerRadius: Radius.md))
            .background(
                RoundedRectangle(cornerRadius: Radius.md).fill(Tokens.popover.color)
                    .shadow(.md, Opacity.shadowStronger)
            )
            .overlay(RoundedRectangle(cornerRadius: Radius.md).strokeBorder(Tokens.borderPopover.color, lineWidth: Theme.hairline))
            .environment(\.onPopover, true)
    }
}

private struct OnPopoverKey: EnvironmentKey {
    static let defaultValue = false
}

extension EnvironmentValues {
    /// Whether the view is on a `PopupCard` or a `Banner`, whose rows and buttons hover in their fills.
    var onPopover: Bool {
        get { self[OnPopoverKey.self] }
        set { self[OnPopoverKey.self] = newValue }
    }
}
