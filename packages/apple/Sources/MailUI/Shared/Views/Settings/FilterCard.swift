import SwiftUI

#if os(macOS)
/// The connect sheet's filter on the Mac: a line of chips for what the app only reaches and one
/// for what it never does, how much mail that comes to, and the menu that adds a condition or
/// changes one, under the chip that opened it.
struct FilterCard: View {
    let draft: AccessDraft
    @Environment(MailStore.self) private var store
    @State private var menu: FilterMenu?
    @State private var tracker = PopupTracker()

    var body: some View {
        SettingsCard {
            line(.only)
            SettingsDivider()
            line(.never)
            SettingsDivider()
            reach
        }
        .onChange(of: menu?.id) { draft.menuOpen = menu != nil }
        .onDisappear { draft.menuOpen = false }
    }

    private func line(_ line: FilterLine) -> some View {
        let chips = draft.summary?.chips(line) ?? []
        return HStack(alignment: .top, spacing: Space.m) {
            Text(line == .only ? "Only" : "Never").textStyle(.label, color: Tokens.mutedForeground.color)
                .frame(width: 48, height: Theme.chipHeight * Platform.scale, alignment: .leading)
            FlowLayout(spacing: Space.xs, lineSpacing: Space.xs, stretchesLast: false) {
                ForEach(chips) { chip in
                    if let joiner = chip.joiner {
                        Text(joiner).textStyle(.footnote, color: Tokens.mutedStrongerForeground.color)
                            .frame(height: Theme.chipHeight * Platform.scale)
                    }
                    let opens = menu?.line == line && menu?.editing?.id == chip.id
                    ConditionChip(label: chip.label, text: chip.text, help: chip.help, empty: chip.empty, open: opens) {
                        toggle(FilterMenu(line: line, editing: chip), opens: opens)
                    } remove: {
                        menu = nil
                        draft.refresh(store, edit: FilterEdit(line: line, remove: chip.filterValue))
                    }
                    .modifier(MenuAnchor(menu: opens ? menu : nil, tracker: tracker, popup: popup))
                }
                let adds = menu?.line == line && menu?.editing == nil
                AddChip(title: line == .only && chips.isEmpty ? "Narrow it" : "Add", open: adds) {
                    toggle(FilterMenu(line: line, editing: nil), opens: adds)
                }
                .modifier(MenuAnchor(menu: adds ? menu : nil, tracker: tracker, popup: popup))
                if chips.isEmpty, let summary = draft.summary {
                    Text(line == .only ? summary.onlyEmpty : summary.neverEmpty)
                        .textStyle(.footnote, color: Tokens.mutedStrongerForeground.color)
                        .frame(height: Theme.chipHeight * Platform.scale)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, Space.l)
        .padding(.vertical, Space.m)
    }

    private var reach: some View {
        VStack(alignment: .leading, spacing: Space.s) {
            HStack(spacing: Space.m) {
                ReachBar(fraction: draft.summary?.fraction ?? 0)
                Text(draft.summary?.reach ?? " ").textStyle(.caption).lineLimit(1)
                Spacer(minLength: 0)
            }
            if let warning = draft.summary?.warning {
                HStack(alignment: .top, spacing: Space.xs) {
                    Image(.circleAlert, size: 12).foregroundStyle(Tokens.warning.color).frame(height: 16 * Platform.scale)
                    Text(warning).textStyle(.caption, color: Tokens.warning.color).fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .padding(.horizontal, Space.l)
        .padding(.vertical, Space.m)
    }

    /// Opens a menu, or closes it when the chip that opened it is clicked again.
    private func toggle(_ new: FilterMenu, opens: Bool) {
        menu = opens ? nil : new
    }

    private func popup(_ menu: FilterMenu) -> AnyView {
        AnyView(
            FilterMenuPopup(menu: menu, access: draft.access) { option in
                pick(option, in: menu)
            } remove: {
                guard let editing = menu.editing else { return }
                self.menu = nil
                draft.refresh(store, edit: FilterEdit(line: menu.line, remove: editing.filterValue))
            } close: {
                self.menu = nil
            }
            .background(PopupPart(role: .popup, tracker: tracker) { self.menu = nil })
            .environment(store)
        )
    }

    private func pick(_ option: FilterOption, in menu: FilterMenu) {
        guard let edit = menu.edit(for: option) else { return menu.narrow(to: option.kind) }
        self.menu = nil
        draft.refresh(store, edit: edit)
    }
}

/// Hangs the menu under the chip that opened it, for the sheet's popup host to draw over
/// everything; that chip is the menu's face, which a click outside the menu doesn't count.
private struct MenuAnchor: ViewModifier {
    let menu: FilterMenu?
    let tracker: PopupTracker
    let popup: (FilterMenu) -> AnyView

    func body(content: Content) -> some View {
        content
            .background {
                if menu != nil { PopupPart(role: .face, tracker: tracker) }
            }
            .anchorPreference(key: ComboboxPopups.self, value: .bounds) { anchor in
                guard let menu else { return [] }
                return [
                    ComboboxPopup(
                        id: menu.id, anchor: anchor, height: FilterMenuPopup.height(of: menu), view: popup(menu),
                        tallest: FilterMenuPopup.tallest(of: menu)
                    )
                ]
            }
    }
}

/// The menu in its popup: the field, the options, and Remove when it changes a chip.
private struct FilterMenuPopup: View {
    @Bindable var menu: FilterMenu
    let access: Access
    let pick: (FilterOption) -> Void
    let remove: () -> Void
    let close: () -> Void
    @Environment(MailStore.self) private var store
    @Environment(\.popupAbove) private var above
    @Environment(\.popupRoom) private var room

    /// Over its chip, the field is at the bottom, next to the chip, and the list grows up from it:
    /// what is typed never moves as the list changes.
    var body: some View {
        PopupCard(inset: 0) {
            if above {
                FilterMenuList(menu: menu, pick: pick)
                footer.rule(.top, color: Tokens.borderPopover)
                field.rule(.top, color: Tokens.borderPopover)
            } else {
                field
                FilterMenuList(menu: menu, pick: pick)
                footer.rule(.top, color: Tokens.borderPopover)
            }
        }
        .frame(width: Theme.popupWidth, height: min(Self.height(of: menu), room))
        .task(id: "\(menu.scope?.rawValue ?? "")|\(menu.query)") { await menu.load(store, access: access) }
    }

    private var field: some View {
        SearchField(
            placeholder: menu.options?.placeholder ?? "", text: $menu.query, style: .popup,
            tag: menu.scope == nil ? nil : menu.options?.scope, clearTag: { menu.narrow(to: nil) },
            deleteEmpty: menu.deleteEmpty, under: above,
            submit: { if menu.picks.indices.contains(menu.highlighted) { pick(menu.picks[menu.highlighted]) } },
            move: menu.move,
            escape: { if menu.escape() { close() } }
        )
    }

    @ViewBuilder private var footer: some View {
        if menu.editing != nil {
            HStack(spacing: Space.s) {
                Text("Pick another to replace it").textStyle(.footnote, color: Tokens.mutedStrongerForeground.color)
                Spacer(minLength: 0)
                ActionButton(title: "Remove", symbol: .trash, variant: .destructive, size: .small, action: remove)
            }
            .padding(.leading, Space.m)
            .padding(.trailing, Space.s)
            .frame(height: ControlSize.regular.height)
        }
    }

    /// The most it grows to: its field, as many rows as it shows before it scrolls, and Remove.
    static func tallest(of menu: FilterMenu) -> CGFloat {
        let footer: CGFloat = menu.editing == nil ? 0 : ControlSize.regular.height
        return ControlSize.regular.height + 8 * Theme.rowHeight * Platform.scale + 2 * Space.s + footer
    }

    /// As tall as its field, its options and their titles (each as tall as the list draws it), up
    /// to the most it shows before it scrolls, and Remove when it changes a chip.
    static func height(of menu: FilterMenu) -> CGFloat {
        let sections = menu.options?.sections ?? []
        let rows = sections.reduce(CGFloat(0)) { total, section in
            let titled = section.title == nil ? 0 : FilterMenuList.titleHeight
            return total + titled + section.options.reduce(0) { $0 + FilterMenuList.rowHeight(of: $1) }
        }
        let note = menu.options?.note == nil ? 0 : FilterMenuList.noteHeight
        let list = min(max(rows + note, Theme.rowHeight * Platform.scale), 8 * Theme.rowHeight * Platform.scale) + 2 * Space.s
        let footer: CGFloat = menu.editing == nil ? 0 : ControlSize.regular.height
        return ControlSize.regular.height + list + footer
    }
}
#endif

/// The options of the filter's menu, in their sections, the highlighted one scrolled to.
struct FilterMenuList: View {
    let menu: FilterMenu
    let pick: (FilterOption) -> Void

    static let titleHeight: CGFloat = 28 * Platform.scale
    static let noteHeight: CGFloat = Theme.rowHeight * Platform.scale

    static func rowHeight(of option: FilterOption) -> CGFloat {
        (twoLines(option) ? Theme.rowHeight + 8 : Theme.rowHeight) * Platform.scale
    }

    /// An option with a count and a detail has the detail on a line of its own, under its title.
    static func twoLines(_ option: FilterOption) -> Bool {
        option.detail != nil && option.count != nil
    }

    var body: some View {
        ScrollViewReader { scroller in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    ForEach(menu.options?.sections ?? []) { section in
                        if let title = section.title {
                            Text(title.uppercased())
                                .textStyle(.footnote, color: Tokens.mutedStrongerForeground.color)
                                .fontWeight(.medium)
                                .padding(.horizontal, Space.s + 2)
                                .padding(.bottom, Space.xxs)
                                .frame(height: Self.titleHeight, alignment: .bottomLeading)
                        }
                        ForEach(section.options) { option in
                            let index = menu.picks.firstIndex(of: option) ?? 0
                            row(option, highlighted: index == menu.highlighted)
                                .frame(height: Self.rowHeight(of: option))
                                .id(option.id)
                                .onHover { inside in if inside { menu.highlighted = index } }
                        }
                    }
                    if let note = menu.options?.note {
                        Text(note).textStyle(.label, color: Tokens.mutedStrongerForeground.color)
                            .lineLimit(1)
                            .padding(.horizontal, Space.s + 2)
                            .frame(maxWidth: .infinity, minHeight: Self.noteHeight, maxHeight: Self.noteHeight, alignment: .leading)
                    }
                }
                .padding(Space.s)
            }
            .onChange(of: menu.keyed) {
                guard menu.picks.indices.contains(menu.highlighted) else { return }
                scroller.scrollTo(menu.picks[menu.highlighted].id)
            }
        }
    }

    private func row(_ option: FilterOption, highlighted: Bool) -> some View {
        let symbol = option.avatar == nil ? option.symbol.map(Symbol.named) : nil
        let twoLines = Self.twoLines(option)
        return ChoiceRow(
            title: option.title,
            subtitle: twoLines ? option.detail : nil,
            detail: option.count ?? option.detail,
            symbol: symbol,
            avatar: option.avatar,
            checked: option.checked,
            highlighted: highlighted
        ) {
            pick(option)
        }
    }
}

/// How much of the mail an access reaches, as a thin bar.
struct ReachBar: View {
    let fraction: Double

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                Capsule().fill(Tokens.backgroundSecondary.color)
                Capsule().fill(Tokens.mutedForeground.color)
                    .frame(width: max(fraction > 0 ? 4 : 0, geometry.size.width * min(max(fraction, 0), 1)))
            }
        }
        .frame(width: 120, height: 4)
        .animation(.easeOut(duration: 0.2), value: fraction)
    }
}
