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
    /// What it may do: "Organizer · Every account · Reads whole messages".
    let access: String
    /// "Approved Nov 5 · Used 5 minutes ago".
    let used: String
}

/// An application asking to reach the user's mail, with the choices of what it may do.
struct ConnectRequest: Codable {
    let key: String
    let client: String
    let verified: Bool
    let publisher: String
    let backTo: String
    let onDevice: Bool
    let accounts: [AccessChoice]
    let roles: [AccessChoice]
    let privileges: [AccessChoice]
}

struct AccessChoice: Codable, Identifiable, Hashable {
    let id: String
    let title: String
    let detail: String
    let symbol: String?
    let color: String?
}

/// A `wonnet://connect` link that opened the app, for the sheet that asks about it.
struct Connecting: Identifiable {
    let id = UUID()
    let url: String
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

    func connectRequest(_ url: String) async throws -> ConnectRequest {
        try await bridge.call("connect_request", ["url": url], as: ConnectRequest.self)
    }

    func approve(_ request: ConnectRequest, access: [String: Any]) async throws {
        _ = try await bridge.call("answer_connect_request", ["key": request.key, "approve": true, "access": access], as: Empty.self)
    }

    func deny(_ request: ConnectRequest) async throws {
        _ = try await bridge.call("answer_connect_request", ["key": request.key, "approve": false], as: Empty.self)
    }

    /// A request left without an answer is denied, so the browser that asked doesn't wait on it.
    func denyQuietly(_ request: ConnectRequest) {
        bridge.send("answer_connect_request", ["key": request.key, "approve": false])
    }
}
