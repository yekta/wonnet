import SwiftUI

/// The access the connect sheet is choosing, and what the core says it comes to. Each change asks
/// the core again, one after the other, so an edit made while one is on its way builds on it.
@MainActor @Observable
final class AccessDraft {
    let request: ConnectRequest
    private(set) var access: Access
    private(set) var summary: AccessSummary?
    var failure: String?
    /// The filter's menu is open: Return picks in it, not Approve.
    var menuOpen = false
    private var last: Task<Void, Never>?

    init(request: ConnectRequest) {
        self.request = request
        self.access = request.access
    }

    var editing: Bool { request.app != nil }

    /// Changes the access here at once, then asks the core what it comes to.
    func change(_ store: MailStore, _ change: (inout Access) -> Void) {
        change(&access)
        refresh(store)
    }

    /// Asks the core what the access comes to after `edit`, which it applies.
    func refresh(_ store: MailStore, edit: FilterEdit? = nil) {
        let previous = last
        last = Task {
            await previous?.value
            do {
                let summary = try await store.accessSummary(client: request.client, access: access, edit: edit)
                self.summary = summary
                self.access = summary.access
                failure = nil
            } catch {
                failure = error.localizedDescription
            }
        }
    }

    /// Waits for the changes on their way, for Approve to send the access they made.
    func settled() async {
        await last?.value
    }

    func toggle(_ id: String, in keyPath: WritableKeyPath<Access, [String]>, _ store: MailStore) {
        change(store) { access in
            if access[keyPath: keyPath].contains(id) {
                access[keyPath: keyPath].removeAll { $0 == id }
            } else {
                access[keyPath: keyPath].append(id)
            }
        }
    }
}

/// The filter's menu: what it adds to or changes (`editing`), the kind picked and the words typed,
/// and the options the core found for them. Only the answer to the latest ask is kept, and the
/// options on screen stay until it comes.
@MainActor @Observable
final class FilterMenu: Identifiable {
    let id = UUID()
    let line: FilterLine
    let editing: FilterChip?
    var scope: FilterKind?
    var query = ""
    private(set) var options: FilterOptions?
    var highlighted = 0
    /// Counts the arrow keys' moves: only they scroll the list to the highlight, not a hover.
    private(set) var keyed = 0
    private var asked = 0

    init(line: FilterLine, editing: FilterChip?) {
        self.line = line
        self.editing = editing
        self.scope = editing?.kind
    }

    var picks: [FilterOption] { options?.options ?? [] }

    var title: String {
        if editing != nil { return "Change" }
        return line == .only ? "Narrow it" : "Keep out"
    }

    func load(_ store: MailStore, access: Access) async {
        asked += 1
        let mine = asked
        let first = options == nil
        do {
            let found = try await store.filterOptions(
                access: access, line: line, kind: scope, query: query, editing: editing?.filterValue
            )
            guard mine == asked else { return }
            options = found
            let checked = found.options.firstIndex(where: \.checked)
            highlighted = first ? (checked ?? 0) : 0
        } catch {
            guard mine == asked else { return }
            store.show(error.localizedDescription)
        }
    }

    /// Narrows the menu to a kind, or takes the kind away.
    func narrow(to kind: FilterKind?) {
        scope = kind
        query = ""
        highlighted = 0
    }

    func move(_ step: Int) {
        highlighted = highlighted.moved(by: step, in: picks.count)
        keyed += 1
    }

    /// Esc: the words typed go first, then the kind picked (when adding), then the menu.
    /// True when the menu should close.
    func escape() -> Bool {
        if !query.isEmpty {
            query = ""
            return false
        }
        if scope != nil && editing == nil {
            narrow(to: nil)
            return false
        }
        return true
    }

    /// Backspace with nothing typed takes the kind picked away, when adding.
    func deleteEmpty() -> Bool {
        guard query.isEmpty, scope != nil, editing == nil else { return false }
        narrow(to: nil)
        return true
    }

    /// The edit an option makes: a kind narrows the menu (nil), a value adds or replaces.
    func edit(for option: FilterOption) -> FilterEdit? {
        guard let value = option.value else { return nil }
        return FilterEdit(line: line, remove: editing?.filterValue, add: FilterValue(kind: option.kind, value: value))
    }
}
