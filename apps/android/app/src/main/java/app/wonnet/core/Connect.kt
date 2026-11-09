package app.wonnet.core

import androidx.core.net.toUri
import java.util.UUID

/** A `wonnet://connect` link that arrived, for the consent sheet. */
data class ConnectLink(val url: String, val id: String = UUID.randomUUID().toString()) {
    /** The request's key, read from the link while the request itself loads. */
    val key: String? get() = url.toUri().getQueryParameter("key")
}

/** An application the user let in, with what it may do and when it was last used. */
data class ConnectedApp(val id: String, val name: String, val verified: Boolean, val access: String, val used: String)

data class ConnectedApps(val address: String, val apps: List<ConnectedApp>) {
    companion object {
        fun read(json: JSON) = ConnectedApps(
            json.string("address"),
            json.objects("apps").map { ConnectedApp(it.string("id"), it.string("name"), it.bool("verified"), it.string("access"), it.string("used")) },
        )
    }
}

/** One choice of what an application may do: an account (with its `color`), a role or a privilege (with its `symbol`). */
data class AccessChoice(val id: String, val title: String, val detail: String, val symbol: String?, val color: String?) {
    companion object {
        fun read(json: JSON) = AccessChoice(json.string("id"), json.string("title"), json.string("detail"), json.optionalString("symbol"), json.optionalString("color"))
    }
}

/** An application asking to connect, and the choices of what it may do. */
data class ConnectRequest(
    val key: String,
    val client: String,
    val verified: Boolean,
    val publisher: String,
    val backTo: String,
    val onDevice: Boolean,
    val accounts: List<AccessChoice>,
    val roles: List<AccessChoice>,
    val privileges: List<AccessChoice>,
) {
    companion object {
        fun read(json: JSON) = ConnectRequest(
            json.string("key"), json.string("client"), json.bool("verified"), json.string("publisher"), json.string("back_to"),
            json.bool("on_device"), json.objects("accounts").map(AccessChoice::read), json.objects("roles").map(AccessChoice::read),
            json.objects("privileges").map(AccessChoice::read),
        )
    }
}

/** What the user lets an application do: a role, every account or only `accounts`, and the privileges. */
data class ConnectAccess(val role: String, val allAccounts: Boolean, val accounts: Set<String>, val privileges: Set<String>) {
    val json: JSON get() = json("role" to role, "all_accounts" to allAccounts, "accounts" to accounts.toList(), "privileges" to privileges.toList())
}

/** Opens the consent sheet for a `wonnet://connect` link, over whatever is on screen. */
fun MailStore.openConnect(url: String) {
    connecting = ConnectLink(url)
}

/** The address of the MCP server applications connect to, at once. */
suspend fun MailStore.mcpServer(): String = bridge.call("mcp_server").string("address")

suspend fun MailStore.connectedApps(): ConnectedApps = bridge.call("connected_apps") { ConnectedApps.read(it.objectOrEmpty) }

suspend fun MailStore.revokeConnectedApp(id: String) {
    bridge.call("revoke_connected_app", mapOf("app" to id))
}

suspend fun MailStore.connectRequest(url: String): ConnectRequest = bridge.call("connect_request", mapOf("url" to url)) { ConnectRequest.read(it.objectOrEmpty) }

suspend fun MailStore.approveConnectRequest(key: String, access: ConnectAccess) {
    bridge.call("answer_connect_request", mapOf("key" to key, "approve" to true, "access" to access.json))
}

/** Denies a request without waiting: the browser hears at once instead of after it gives up. */
fun MailStore.denyConnectRequest(key: String) = bridge.send("answer_connect_request", mapOf("key" to key, "approve" to false))
