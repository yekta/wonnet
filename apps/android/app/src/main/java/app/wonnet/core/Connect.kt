package app.wonnet.core

import androidx.core.net.toUri
import kotlinx.serialization.json.JsonObject
import java.util.UUID

/** What the consent sheet opens: a `wonnet://connect` link that arrived, or a connected `app` to change. */
data class ConnectLink(val url: String = "", val app: String? = null, val id: String = UUID.randomUUID().toString()) {
    /** The request's key, read from the link while the request itself loads. */
    val key: String? get() = if (app != null) null else url.toUri().getQueryParameter("key")
}

/** An application the user let in, with what it may do in a sentence and when it was last used. */
data class ConnectedApp(val id: String, val name: String, val verified: Boolean, val access: String, val used: String)

data class ConnectedApps(val address: String, val apps: List<ConnectedApp>) {
    companion object {
        fun read(json: JSON) = ConnectedApps(
            json.string("address"),
            json.objects("apps").map { ConnectedApp(it.string("id"), it.string("name"), it.bool("verified"), it.string("access"), it.string("used")) },
        )
    }
}

/** One choice of what an application may do: an account (with its `color`), a part it reads, a role or a privilege (with its `symbol`). */
data class AccessChoice(val id: String, val title: String, val detail: String, val symbol: String?, val color: String?) {
    companion object {
        fun read(json: JSON) = AccessChoice(json.string("id"), json.string("title"), json.string("detail"), json.optionalString("symbol"), json.optionalString("color"))
    }
}

/**
 * What an application may reach and do, as the core writes it (`mail_protocol::access::Access`):
 * kept whole and sent back whole, so the filter's lines go along untouched.
 */
data class Access(val json: JSON) {
    val role: String get() = json.string("role")
    val allAccounts: Boolean get() = json.bool("all_accounts")
    val accounts: List<String> get() = json.strings("accounts")
    val reads: List<String> get() = json.strings("reads")
    val privileges: List<String> get() = json.strings("privileges")

    private fun with(key: String, value: Any): Access = Access(JsonObject(json + (key to jsonOf(value))))

    fun withRole(role: String) = with("role", role)

    fun withAllAccounts(all: Boolean) = with("all_accounts", all)

    fun toggling(account: String) = with("accounts", if (account in accounts) accounts - account else accounts + account)

    fun togglingRead(part: String) = with("reads", if (part in reads) reads - part else reads + part)

    fun togglingPrivilege(privilege: String) = with("privileges", if (privilege in privileges) privileges - privilege else privileges + privilege)
}

/** An application asking to connect, or a connected `app` being changed, the choices of what it may do, and the access the sheet starts with. */
data class ConnectRequest(
    val key: String,
    val app: String?,
    val client: String,
    val verified: Boolean,
    val publisher: String,
    val backTo: String,
    val onDevice: Boolean,
    val accounts: List<AccessChoice>,
    val always: AccessChoice,
    val reads: List<AccessChoice>,
    val roles: List<AccessChoice>,
    val privileges: List<AccessChoice>,
    val access: Access,
) {
    companion object {
        fun read(json: JSON) = ConnectRequest(
            json.string("key"), json.optionalString("app"), json.string("client"), json.bool("verified"), json.string("publisher"),
            json.string("back_to"), json.bool("on_device"), json.objects("accounts").map(AccessChoice::read),
            AccessChoice.read(json.obj("always") ?: EmptyJSON), json.objects("reads").map(AccessChoice::read),
            json.objects("roles").map(AccessChoice::read), json.objects("privileges").map(AccessChoice::read), Access(json.obj("access") ?: EmptyJSON),
        )
    }
}

/** A line of the filter: `Only` narrows what an app reaches, `Never` keeps mail from it. */
enum class FilterLine(val id: String) { Only("only"), Never("never") }

/** One condition: its kind (`from`, `to`, `label`, `subject`, `body`, `attachment`, `date`) and its value. */
data class FilterValue(val kind: String, val value: String) {
    val json: JSON get() = json("kind" to kind, "value" to value)
}

/** A change to a line of the filter: takes `remove` away and puts `add` in, either or both. */
data class FilterEdit(val line: FilterLine, val remove: FilterValue? = null, val add: FilterValue? = null) {
    val json: JSON get() = json("line" to line.id, "remove" to remove?.json, "add" to add?.json)
}

/** A run of the sentence, in bold when `strong`. */
data class SentencePart(val text: String, val strong: Boolean)

/** A condition as the sheet draws it: `label` its kind's word ("from"), `text` its value, `joiner` the "or" or "and" before it; none of the mail `matches` is a warning. */
data class FilterChip(val kind: String, val value: String, val label: String, val text: String, val joiner: String?, val matches: Int, val help: String) {
    val filterValue: FilterValue get() = FilterValue(kind, value)

    companion object {
        fun read(json: JSON) = FilterChip(
            json.string("kind"), json.string("value"), json.string("label"), json.string("text"), json.optionalString("joiner"), json.int("matches"),
            json.string("help"),
        )
    }
}

/** What the sheet draws about an access: the access to keep from now on, its sentence, how much it reaches, each line's chips, and why it can't be approved yet. */
data class AccessSummary(
    val access: Access,
    val sentence: List<SentencePart>,
    val reach: String,
    val fraction: Double,
    val only: List<FilterChip>,
    val never: List<FilterChip>,
    val onlyEmpty: String,
    val neverEmpty: String,
    val warning: String?,
    val problem: String?,
) {
    fun chips(line: FilterLine) = if (line == FilterLine.Only) only else never

    companion object {
        fun read(json: JSON) = AccessSummary(
            Access(json.obj("access") ?: EmptyJSON), json.objects("sentence").map { SentencePart(it.string("text"), it.bool("strong")) },
            json.string("reach"), json.double("fraction"), json.objects("only").map(FilterChip::read), json.objects("never").map(FilterChip::read),
            json.string("only_empty"), json.string("never_empty"), json.optionalString("warning"), json.optionalString("problem"),
        )
    }
}

/** One option of the filter's menu: without a `value` it picks its `kind`; with one it is a condition to add, or to change the chip to. */
data class FilterOption(
    val id: String,
    val kind: String,
    val value: String?,
    val title: String,
    val detail: String?,
    val symbol: String?,
    val avatar: Address?,
    val count: String?,
    val checked: Boolean,
) {
    companion object {
        fun read(json: JSON) = FilterOption(
            json.string("id"), json.string("kind"), json.optionalString("value"), json.string("title"), json.optionalString("detail"),
            json.optionalString("symbol"), json.obj("avatar")?.let(Address::read), json.optionalString("count"), json.bool("checked"),
        )
    }
}

data class FilterSection(val title: String?, val options: List<FilterOption>)

/** The filter's menu: the field's placeholder, the kind picked (`scope`), the options in sections, and a note when there is nothing to pick. */
data class FilterOptions(val placeholder: String, val scope: String?, val sections: List<FilterSection>, val note: String?) {
    companion object {
        fun read(json: JSON) = FilterOptions(
            json.string("placeholder"), json.optionalString("scope"),
            json.objects("sections").map { section -> FilterSection(section.optionalString("title"), section.objects("options").map(FilterOption::read)) },
            json.optionalString("note"),
        )
    }
}

/** Opens the consent sheet for a `wonnet://connect` link, over whatever is on screen. */
fun MailStore.openConnect(url: String) {
    connecting = ConnectLink(url)
}

/** Opens the same sheet for a connected app, to change what it may do. */
fun MailStore.changeConnectedApp(app: ConnectedApp) {
    connecting = ConnectLink(app = app.id)
}

/** The address of the MCP server applications connect to, at once. */
suspend fun MailStore.mcpServer(): String = bridge.call("mcp_server").string("address")

suspend fun MailStore.connectedApps(): ConnectedApps = bridge.call("connected_apps") { ConnectedApps.read(it.objectOrEmpty) }

suspend fun MailStore.revokeConnectedApp(id: String) {
    bridge.call("revoke_connected_app", mapOf("app" to id))
}

/** What the sheet opens: the request a link is about, or a connected app. */
suspend fun MailStore.connectRequest(link: ConnectLink): ConnectRequest {
    val app = link.app
    if (app != null) return bridge.call("edit_connected_app", mapOf("app" to app)) { ConnectRequest.read(it.objectOrEmpty) }
    return bridge.call("connect_request", mapOf("url" to link.url)) { ConnectRequest.read(it.objectOrEmpty) }
}

suspend fun MailStore.accessSummary(client: String, access: Access, edit: FilterEdit?): AccessSummary =
    bridge.call("access_summary", mapOf("client" to client, "access" to access.json, "edit" to edit?.json)) { AccessSummary.read(it.objectOrEmpty) }

suspend fun MailStore.filterOptions(access: Access, line: FilterLine, kind: String?, query: String, editing: FilterValue?): FilterOptions =
    bridge.call(
        "filter_options", mapOf("access" to access.json, "line" to line.id, "kind" to kind, "query" to query, "editing" to editing?.json),
    ) { FilterOptions.read(it.objectOrEmpty) }

suspend fun MailStore.approveConnectRequest(key: String, access: Access) {
    bridge.call("answer_connect_request", mapOf("key" to key, "approve" to true, "access" to access.json))
}

suspend fun MailStore.setConnectedAppAccess(app: String, access: Access) {
    bridge.call("set_connected_app_access", mapOf("app" to app, "access" to access.json))
}

/** Denies a request without waiting: the browser hears at once instead of after it gives up. */
fun MailStore.denyConnectRequest(key: String) = bridge.send("answer_connect_request", mapOf("key" to key, "approve" to false))
