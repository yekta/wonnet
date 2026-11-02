import SwiftUI

/// A section's name, and maybe a muted line about it.
struct SectionHeading: View {
    let title: String
    var detail: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Space.xxs) {
            Text(title).textStyle(.subheading)
            if let detail {
                Text(detail).textStyle(.caption, color: Tokens.mutedForeground.color).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// A part of a page: its heading, then its controls.
struct FormSection<Content: View>: View {
    let title: String
    var detail: String?
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: Space.m) {
            SectionHeading(title: title, detail: detail)
            content
        }
    }
}

/// A rounded card on the page: a form being filled, a thing of its own.
struct Card<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        content
            .padding(Space.l)
            .background(RoundedRectangle(cornerRadius: Radius.md).fill(Tokens.card.color))
            .overlay(RoundedRectangle(cornerRadius: Radius.md).strokeBorder(Tokens.border.color, lineWidth: Theme.hairline))
    }
}

extension View {
    /// The card that fills the page (`Theme.cardFill`), with what is on it told so.
    func pageCard() -> some View {
        background(RoundedRectangle(cornerRadius: Theme.cardRadius).fill(Theme.cardFill.color).shadow(.lg))
            .overlay(RoundedRectangle(cornerRadius: Theme.cardRadius).strokeBorder(Tokens.border.color, lineWidth: Theme.hairline))
            .environment(\.onCard, true)
    }
}

private struct OnCardKey: EnvironmentKey {
    static let defaultValue = false
}

extension EnvironmentValues {
    /// Whether the view is on the card that fills the page, where a chip fills with its accents.
    var onCard: Bool {
        get { self[OnCardKey.self] }
        set { self[OnCardKey.self] = newValue }
    }
}

/// Cancel and Save under a form.
struct FormButtons: View {
    var save = "Save"
    var saveDisabled = false
    let onSave: () -> Void
    let onCancel: () -> Void

    var body: some View {
        HStack(spacing: 0) {
            ActionButton(title: "Cancel", variant: .ghost, action: onCancel).keyboardShortcut(.cancelAction)
            ActionButton(title: save, variant: .primary, action: onSave).disabled(saveDisabled)
        }
    }
}
