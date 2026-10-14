import SwiftUI

/// A field that narrows a list: the search icon, the words, and a way to clear them. `.sheet`
/// sits on a hairline over a sheet's list, given by `SearchedList`, and `.popup` over a popup's,
/// smaller; both take the keys that move through the list (↑, ↓, Enter to pick, Esc to leave).
/// `.bar` is the capsule of the top bar.
struct SearchField: View {
    enum Style {
        case sheet, popup, bar
    }

    let placeholder: String
    @Binding var text: String
    var style: Style = .sheet
    var symbol: Symbol = .search
    var autofocus = true
    /// Changed by the owner to put the cursor in the field again.
    var focusTrigger = 0
    var submit: () -> Void = {}
    var move: (Int) -> Void = { _ in }
    var escape: () -> Void = {}
    var clear: () -> Void = {}
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: Space.s) {
            Image(symbol, size: 14).foregroundStyle(Tokens.mutedStrongerForeground.color).allowsHitTesting(false)
            TextField(placeholder, text: $text)
                .textFieldStyle(.plain)
                .font(.ui(style == .sheet ? 15 : 13))
                .foregroundStyle(Tokens.foreground.color)
                .focused($focused)
                .onSubmit(submit)
                .onListKeys(move: move, escape: leave)
                #if os(iOS)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                #endif
            if !text.isEmpty {
                IconButton(symbol: .x, help: "Clear", size: .small, circled: false, quiet: true, action: clearText)
            }
        }
        .padding(.leading, Space.m)
        .padding(.trailing, text.isEmpty ? Space.m : clearInset)
        .frame(height: height)
        .background { FieldArea { focused = true } }
        .background { if style == .bar { Capsule().strokeBorder(Tokens.borderInput.color, lineWidth: Theme.hairline) } }
        .overlay(alignment: .bottom) {
            if let rule { Rule(color: rule) }
        }
        .onAppear { if autofocus { focused = true } }
        .onChange(of: focusTrigger) { focused = true }
    }

    private var height: CGFloat {
        (style == .sheet ? Theme.searchHeight : ControlSize.regular.height) * Platform.scale
    }

    private var rule: ThemeColor? {
        switch style {
        case .sheet: Tokens.borderCard
        case .popup: Tokens.borderPopover
        case .bar: nil
        }
    }

    /// Clear sits as far from the end as from the top and the bottom.
    private var clearInset: CGFloat {
        (height - ControlSize.small.height * Platform.scale) / 2 - Theme.buttonGap / 2
    }

    private func clearText() {
        text = ""
        clear()
    }

    private func leave() {
        escape()
        guard style == .bar else { return }
        clearText()
        focused = false
    }
}

/// A sheet's list and the field that narrows it: ours over the list on the Mac, the system's
/// search box along the bottom on iOS, where the thumb is. It is not focused on iOS: the sheet
/// comes up in one move and the keyboard is a tap away.
struct SearchedList<List: View>: View {
    let field: SearchField
    @ViewBuilder let list: List

    var body: some View {
        #if os(macOS)
        VStack(spacing: 0) {
            field
            list
        }
        #else
        list
            .searchField(text: field.$text, prompt: field.placeholder)
            .onSubmit(of: .search, field.submit)
        #endif
    }
}
