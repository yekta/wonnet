import SwiftUI

/// One thing a `Combobox` offers: what it is called, a word about it at the end of its row, and
/// what its row starts with. `keywords` is more it is found by, such as its account's name.
struct Choice: Identifiable, Hashable {
    let id: String
    let title: String
    var detail: String?
    var symbol: Symbol?
    var dot: Color?
    var avatar: Address?
    var keywords = ""

    /// Whether every word typed is in its title, its detail or its keywords.
    func matches(_ query: String) -> Bool {
        let haystack = [title, detail ?? "", keywords].joined(separator: " ")
        return query.split(whereSeparator: \.isWhitespace).allSatisfy { haystack.localizedCaseInsensitiveContains($0) }
    }
}

/// A choice from a list that can grow, found by typing a few letters of it: the address a
/// message goes from. Its face shows the choice in force. On the Mac it opens a popup under the
/// face with a field that narrows the list, closed by Esc, a pick or a click elsewhere; on iOS
/// a sheet of the `title`. `.field` is a boxed field with a `label` above it; `.inline` is only
/// its words, for a line of a message's head. The Mac's popup is drawn by the nearest
/// `comboboxHost`, the window's root or the sheet's, so nothing on the page is over it.
struct Combobox: View {
    enum Style {
        case field, inline
    }

    let title: String
    let placeholder: String
    let choices: [Choice]
    let selected: Choice.ID?
    var style: Style = .field
    var label: String?
    var help: String?
    var empty = "Nothing by that name."
    /// Whether the list is open; given by an owner that opens it with a key of its own.
    var open: Binding<Bool>?
    let pick: (Choice) -> Void
    @State private var ownOpen = false
    @State private var query = ""
    @State private var highlighted = 0

    private var isOpen: Binding<Bool> { open ?? $ownOpen }

    /// The most rows the Mac's popup shows before it scrolls.
    private static let shownRows = 8

    private var current: Choice? { choices.first { $0.id == selected } }

    private var found: [Choice] {
        let typed = query.trimmingCharacters(in: .whitespaces)
        guard !typed.isEmpty else { return choices }
        return choices.filter { $0.matches(typed) }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Space.xxs) {
            if let label {
                Text(label).textStyle(.footnote, color: Tokens.mutedStrongerForeground.color).fontWeight(.medium)
            }
            face
        }
        .onChange(of: isOpen.wrappedValue) { _, open in
            guard open else { return }
            query = ""
            highlighted = current.flatMap { choices.firstIndex(of: $0) } ?? 0
        }
        .onChange(of: query) { highlighted = 0 }
        #if os(macOS)
        .anchorPreference(key: ComboboxPopups.self, value: .bounds) { anchor in
            guard isOpen.wrappedValue else { return [] }
            return [ComboboxPopup(id: id, anchor: anchor, height: popupHeight, view: AnyView(popup))]
        }
        #else
        .sheet(isPresented: isOpen) {
            Sheet(title: title) {
                SearchedList(field: field) { list }
            }
        }
        #endif
    }

    @ViewBuilder private var face: some View {
        let button = Button {
            isOpen.wrappedValue.toggle()
        } label: {
            Face(choice: current, placeholder: placeholder, style: style, open: isOpen.wrappedValue)
        }
        .buttonStyle(.press)
        #if os(macOS)
        .background(PopupPart(role: .face, tracker: tracker))
        #endif
        if let help {
            button.help(help).accessibilityLabel(help)
        } else {
            button
        }
    }

    private var field: SearchField {
        #if os(macOS)
        let style = SearchField.Style.popup
        #else
        let style = SearchField.Style.sheet
        #endif
        return SearchField(
            placeholder: placeholder, text: $query, style: style,
            submit: { choose(highlighted) }, move: { highlighted = highlighted.moved(by: $0, in: found.count) },
            escape: { isOpen.wrappedValue = false }
        )
    }

    private var list: some View {
        ChoiceList(items: found, highlighted: highlighted, empty: empty) { index, choice in
            ChoiceRow(
                title: choice.title, detail: choice.detail, symbol: choice.symbol, dot: choice.dot, avatar: choice.avatar,
                checked: choice.id == selected, highlighted: index == highlighted
            ) {
                choose(index)
            }
        }
    }

    private func choose(_ index: Int) {
        let found = found
        guard found.indices.contains(index) else { return }
        pick(found[index])
        isOpen.wrappedValue = false
    }

    #if os(macOS)
    @State private var id = UUID()
    @State private var tracker = PopupTracker()

    /// The field over the list, as tall as its rows up to `shownRows`, then scrolling.
    private var popupHeight: CGFloat {
        let rows = max(min(found.count, Self.shownRows), 1)
        return ControlSize.regular.height + CGFloat(rows) * Theme.rowHeight + 2 * Space.s
    }

    private var popup: some View {
        PopupCard(inset: 0) {
            field
            list
        }
        .frame(width: Theme.popupWidth, height: popupHeight)
        .background(PopupPart(role: .popup, tracker: tracker) { isOpen.wrappedValue = false })
    }
    #endif

    private struct Face: View {
        let choice: Choice?
        let placeholder: String
        let style: Style
        let open: Bool
        @Environment(\.hovered) private var hovered

        var body: some View {
            HStack(spacing: Space.s) {
                if let choice {
                    if let avatar = choice.avatar {
                        Avatar(initials: avatar.initials, email: avatar.email, size: 20)
                    } else if let dot = choice.dot {
                        Circle().fill(dot).frame(width: 8, height: 8)
                    } else if let symbol = choice.symbol {
                        Image(symbol, size: 14).foregroundStyle(Tokens.mutedStrongerForeground.color)
                    }
                    Text(choice.title).textStyle(.body).lineLimit(1)
                    if let detail = choice.detail {
                        Text(detail).textStyle(.caption).lineLimit(1).truncationMode(.middle)
                    }
                } else {
                    Text(placeholder).textStyle(.body, color: Tokens.mutedStrongerForeground.color).lineLimit(1)
                }
                if style == .field { Spacer(minLength: Space.s) }
                Image(.chevronDown, size: 12).foregroundStyle(Tokens.mutedStrongerForeground.color)
            }
            .padding(.horizontal, style == .field ? Space.s + 2 : Space.xxs)
            .frame(minHeight: style == .field ? Theme.fieldHeight * Platform.scale : ControlSize.small.height)
            .background(RoundedRectangle(cornerRadius: Radius.sm).fill(fill))
            .overlay {
                if style == .field {
                    RoundedRectangle(cornerRadius: Radius.sm).strokeBorder(Tokens.borderInput.color, lineWidth: Theme.hairline)
                }
            }
            .contentShape(RoundedRectangle(cornerRadius: Radius.sm))
        }

        private var fill: Color {
            if hovered || open { return Tokens.backgroundAccent.color }
            return style == .field ? Tokens.input.color : .clear
        }
    }
}

#if os(macOS)
/// An open combobox's popup, handed up to the host that draws it: where its face is, how tall
/// it is, and the popup itself.
struct ComboboxPopup {
    let id: UUID
    let anchor: Anchor<CGRect>
    let height: CGFloat
    let view: AnyView
}

struct ComboboxPopups: PreferenceKey {
    static let defaultValue: [ComboboxPopup] = []

    static func reduce(value: inout [ComboboxPopup], nextValue: () -> [ComboboxPopup]) {
        value += nextValue()
    }
}

/// Draws the comboboxes' popups over everything in the view, each under its face, or over it
/// when there is no room below; shifted in from the right edge when it would cross it.
private struct ComboboxHost: ViewModifier {
    func body(content: Content) -> some View {
        content.overlayPreferenceValue(ComboboxPopups.self) { popups in
            GeometryReader { geometry in
                ForEach(popups, id: \.id) { popup in
                    let face = geometry[popup.anchor]
                    let below = face.maxY + Space.xxs + popup.height <= geometry.size.height
                    popup.view.offset(
                        x: min(face.minX, max(geometry.size.width - Theme.popupWidth - Space.s, 0)),
                        y: below ? face.maxY + Space.xxs : face.minY - Space.xxs - popup.height
                    )
                }
            }
        }
    }
}

extension View {
    /// Where the comboboxes in the view draw their popups: the window's root and every sheet.
    func comboboxHost() -> some View {
        modifier(ComboboxHost())
    }
}

/// Where a combobox's face and its popup are, so a click anywhere else closes the popup while
/// one on the face is left to the face, which toggles it.
private final class PopupTracker {
    weak var face: NSView?
    weak var popup: NSView?
    var monitor: Any?
    var outside: () -> Void = {}

    func watch() {
        guard monitor == nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { [weak self] event in
            guard let self, let popup, let window = popup.window, event.window === window else { return event }
            let inside = [popup, face].compactMap { $0 }.contains { view in
                view.bounds.contains(view.convert(event.locationInWindow, from: nil))
            }
            if !inside { outside() }
            return event
        }
    }

    func stop() {
        guard let monitor else { return }
        NSEvent.removeMonitor(monitor)
        self.monitor = nil
    }
}

private struct PopupPart: NSViewRepresentable {
    enum Role {
        case face, popup
    }

    final class Coordinator {
        var stop: () -> Void = {}
    }

    let role: Role
    let tracker: PopupTracker
    var outside: () -> Void = {}

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        switch role {
        case .face:
            tracker.face = view
        case .popup:
            tracker.popup = view
            tracker.watch()
            context.coordinator.stop = { [tracker] in tracker.stop() }
        }
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        guard role == .popup else { return }
        tracker.outside = outside
    }

    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) {
        coordinator.stop()
    }
}
#endif
