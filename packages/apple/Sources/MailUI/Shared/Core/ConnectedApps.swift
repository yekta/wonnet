import SwiftUI

/// The MCP server applications connect to.
struct McpServer: Codable {
    let address: String
}

struct ConnectedAppsList: Codable {
    let address: String
    let apps: [ConnectedApp]
}

/// An application the user let reach their mail.
struct ConnectedApp: Codable, Identifiable, Hashable {
    let id: String
    let name: String
    let verified: Bool
    /// What it may do, in a sentence: "Reads the subject of mail in Work, and only looks."
    let access: String
    /// "Approved Nov 5 · Used 5 minutes ago".
    let used: String
}

/// An application asking to reach the user's mail, or one connected already being changed
/// (`app`), with the choices of what it may do and the access the sheet starts with.
struct ConnectRequest: Codable {
    let key: String
    let app: String?
    let client: String
    let verified: Bool
    let publisher: String
    let backTo: String
    let onDevice: Bool
    let accounts: [AccessChoice]
    let always: AccessChoice
    let reads: [AccessChoice]
    let roles: [AccessChoice]
    let privileges: [AccessChoice]
    let access: Access
}

/// What a connected app may do, as the core and the server keep it (`mail_protocol::access::Access`).
struct Access: Codable, Hashable {
    var role: String
    var allAccounts: Bool
    var accounts: [String]
    var reads: [String]
    var privileges: [String]
    var only: Conditions
    var never: Conditions

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        role = try values.decode(String.self, forKey: .role)
        allAccounts = try values.decode(Bool.self, forKey: .allAccounts)
        accounts = try values.decodeIfPresent([String].self, forKey: .accounts) ?? []
        reads = try values.decodeIfPresent([String].self, forKey: .reads) ?? []
        privileges = try values.decodeIfPresent([String].self, forKey: .privileges) ?? []
        only = try values.decodeIfPresent(Conditions.self, forKey: .only) ?? Conditions()
        never = try values.decodeIfPresent(Conditions.self, forKey: .never) ?? Conditions()
    }
}

/// What mail must be like, kind by kind; the core reads and changes them, the app only keeps them.
struct Conditions: Codable, Hashable {
    var from: [String] = []
    var to: [String] = []
    var labels: [String] = []
    var subject: [String] = []
    var body: [String] = []
    var attachment: [String] = []
    var newerThan: String?
    var olderThan: String?

    init() {}

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        from = try values.decodeIfPresent([String].self, forKey: .from) ?? []
        to = try values.decodeIfPresent([String].self, forKey: .to) ?? []
        labels = try values.decodeIfPresent([String].self, forKey: .labels) ?? []
        subject = try values.decodeIfPresent([String].self, forKey: .subject) ?? []
        body = try values.decodeIfPresent([String].self, forKey: .body) ?? []
        attachment = try values.decodeIfPresent([String].self, forKey: .attachment) ?? []
        newerThan = try values.decodeIfPresent(String.self, forKey: .newerThan)
        olderThan = try values.decodeIfPresent(String.self, forKey: .olderThan)
    }
}

/// A line of the filter: `only` narrows what an app reaches, `never` keeps mail from it.
enum FilterLine: String, Codable {
    case only, never
}

enum FilterKind: String, Codable, Hashable {
    case from, to, label, subject, body, attachment, date

    /// The icon of a condition of this kind, as a row of the iOS line's page shows it.
    var symbol: Symbol {
        switch self {
        case .from: .user
        case .to: .send
        case .label: .tag
        case .subject: .type
        case .body: .text
        case .attachment: .paperclip
        case .date: .calendar
        }
    }
}

struct FilterValue: Codable, Hashable {
    let kind: FilterKind
    let value: String
}

/// A change to a line of the filter: `remove` taken away, `add` put in.
struct FilterEdit {
    let line: FilterLine
    var remove: FilterValue?
    var add: FilterValue?

    var fields: [String: Any] {
        var fields: [String: Any] = ["line": line.rawValue]
        if let remove { fields["remove"] = CoreBridge.object(remove) }
        if let add { fields["add"] = CoreBridge.object(add) }
        return fields
    }
}

/// A run of the sentence that says what an app may do, in bold when `strong`.
struct SentencePart: Codable, Hashable {
    let text: String
    let strong: Bool
}

/// A condition as a chip: its kind's word, its value, the "or" or "and" before it, and how many
/// messages it matches; none makes it a warning.
struct FilterChip: Codable, Hashable, Identifiable {
    let kind: FilterKind
    let value: String
    let label: String
    let text: String
    let joiner: String?
    let matches: Int
    let help: String

    var id: String { "\(kind.rawValue):\(value)" }
    var filterValue: FilterValue { FilterValue(kind: kind, value: value) }
    var empty: Bool { matches == 0 }

    /// "From @chase.com", for a row of its own.
    var title: String { label.isEmpty ? text.prefix(1).uppercased() + text.dropFirst() : "\(label.prefix(1).uppercased())\(label.dropFirst()) \(text)" }
}

/// What the core says about an access while it is chosen.
struct AccessSummary: Codable {
    let access: Access
    let sentence: [SentencePart]
    let reach: String
    let fraction: Double
    let only: [FilterChip]
    let never: [FilterChip]
    let onlyEmpty: String
    let neverEmpty: String
    let warning: String?
    let problem: String?

    func chips(_ line: FilterLine) -> [FilterChip] { line == .only ? only : never }
}

struct FilterOptions: Codable {
    let placeholder: String
    let scope: String?
    let sections: [FilterSection]
    let note: String?

    var options: [FilterOption] { sections.flatMap(\.options) }
}

struct FilterSection: Codable, Hashable, Identifiable {
    let title: String?
    let options: [FilterOption]

    var id: String { (title ?? "") + (options.first?.id ?? "") }
}

/// One option of the filter's menu: a kind to narrow it to (no `value`), or a condition.
struct FilterOption: Codable, Hashable, Identifiable {
    let id: String
    let kind: FilterKind
    let value: String?
    let title: String
    let detail: String?
    let symbol: String?
    let avatar: Address?
    let count: String?
    let checked: Bool
}

struct AccessChoice: Codable, Identifiable, Hashable {
    let id: String
    let title: String
    let detail: String
    let symbol: String?
    let color: String?
}

/// What the connect sheet is about: a `wonnet://connect` link that opened the app, or a
/// connected app whose access is changed.
struct Connecting: Identifiable {
    let id = UUID()
    var url: String?
    var app: String?
}

/// What Settings → Connected apps shows: the MCP server at once, the apps once the server answers.
@MainActor @Observable
final class ConnectedAppsModel {
    var address: String?
    var apps: [ConnectedApp]?
    var error: String?
    var loading = false
    /// The apps being revoked, whose buttons wait.
    var revoking: Set<String> = []

    func load(_ store: MailStore) async {
        address = try? await store.bridge.call("mcp_server", as: McpServer.self).address
        loading = true
        error = nil
        defer { loading = false }
        do {
            let found = try await store.bridge.call("connected_apps", as: ConnectedAppsList.self)
            // A reload, when an app connected: the new row slides in.
            if apps == nil { apps = found.apps } else { withAnimation { apps = found.apps } }
            address = found.address
        } catch {
            self.error = error.localizedDescription
        }
    }

    /// Asks first, then ends the app's access; the row goes once the server has it.
    func confirmRevoking(_ app: ConnectedApp, _ store: MailStore) {
        store.confirmation = Confirmation(
            title: "Revoke \(app.name)?", message: "It can no longer reach your mail. You can connect it again.", action: "Revoke"
        ) { [weak self] in self?.revoke(app, store) }
    }

    func revoke(_ app: ConnectedApp, _ store: MailStore) {
        revoking.insert(app.id)
        Task {
            defer { revoking.remove(app.id) }
            do {
                _ = try await store.bridge.call("revoke_connected_app", ["app": app.id], as: Empty.self)
                withAnimation { apps?.removeAll { $0.id == app.id } }
                store.show("Revoked \(app.name).")
            } catch {
                store.show(error.localizedDescription)
            }
        }
    }
}

extension MailStore {
    /// A `wonnet://` link: an application asking to connect, or the end of a sign-in.
    public func open(_ url: URL) async {
        guard url.host() == "connect" else { return await finishSignIn(url) }
        connecting = Connecting(url: url.absoluteString)
    }

    func connectRequest(_ connecting: Connecting) async throws -> ConnectRequest {
        if let app = connecting.app {
            return try await bridge.call("edit_connected_app", ["app": app], as: ConnectRequest.self)
        }
        return try await bridge.call("connect_request", ["url": connecting.url ?? ""], as: ConnectRequest.self)
    }

    /// Opens the connect sheet on a connected app, to change what it may do.
    func change(_ app: ConnectedApp) {
        connecting = Connecting(app: app.id)
    }

    func approve(_ request: ConnectRequest, access: Access) async throws {
        let fields: [String: Any] = ["key": request.key, "approve": true, "access": CoreBridge.object(access)]
        _ = try await bridge.call("answer_connect_request", fields, as: Empty.self)
    }

    func saveAccess(_ app: String, access: Access) async throws {
        _ = try await bridge.call("set_connected_app_access", ["app": app, "access": CoreBridge.object(access)], as: Empty.self)
    }

    func accessSummary(client: String, access: Access, edit: FilterEdit?) async throws -> AccessSummary {
        var fields: [String: Any] = ["client": client, "access": CoreBridge.object(access)]
        if let edit { fields["edit"] = edit.fields }
        return try await bridge.call("access_summary", fields, as: AccessSummary.self)
    }

    func filterOptions(access: Access, line: FilterLine, kind: FilterKind?, query: String, editing: FilterValue?) async throws -> FilterOptions {
        var fields: [String: Any] = ["access": CoreBridge.object(access), "line": line.rawValue, "query": query]
        if let kind { fields["kind"] = kind.rawValue }
        if let editing { fields["editing"] = CoreBridge.object(editing) }
        return try await bridge.call("filter_options", fields, as: FilterOptions.self)
    }

    func deny(_ request: ConnectRequest) async throws {
        _ = try await bridge.call("answer_connect_request", ["key": request.key, "approve": false], as: Empty.self)
    }

    /// A request left without an answer is denied, so the browser that asked doesn't wait on it.
    func denyQuietly(_ request: ConnectRequest) {
        bridge.send("answer_connect_request", ["key": request.key, "approve": false])
    }
}
