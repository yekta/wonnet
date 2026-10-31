package app.wonnet.core

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.longOrNull

// Reading and writing the core's JSON, whose shapes crates/core/src/api.rs defines.

typealias JSON = JsonObject

val EmptyJSON = JsonObject(emptyMap())

fun parseJSON(text: String): JsonElement? = runCatching { Json.parseToJsonElement(text) }.getOrNull()

private fun JSON.primitive(key: String): JsonPrimitive? = (this[key] as? JsonPrimitive)?.takeIf { it !is JsonNull }

fun JSON.string(key: String): String = optionalString(key) ?: ""

fun JSON.optionalString(key: String): String? = primitive(key)?.takeIf { it.isString }?.content

fun JSON.bool(key: String): Boolean = primitive(key)?.booleanOrNull ?: false

fun JSON.long(key: String): Long = optionalLong(key) ?: 0

fun JSON.optionalLong(key: String): Long? = primitive(key)?.let { it.longOrNull ?: it.doubleOrNull?.toLong() }

fun JSON.int(key: String): Int = long(key).toInt()

fun JSON.double(key: String): Double = primitive(key)?.doubleOrNull ?: 0.0

fun JSON.optionalDouble(key: String): Double? = primitive(key)?.doubleOrNull

fun JSON.obj(key: String): JSON? = this[key] as? JsonObject

fun JSON.objects(key: String): List<JSON> = (this[key] as? JsonArray)?.mapNotNull { it as? JsonObject } ?: emptyList()

fun JSON.strings(key: String): List<String> =
    (this[key] as? JsonArray)?.mapNotNull { (it as? JsonPrimitive)?.takeIf { value -> value.isString }?.content } ?: emptyList()

/** A value's string, when it is one. */
val JsonElement.text: String? get() = (this as? JsonPrimitive)?.takeIf { it.isString }?.content

/** A value's boolean, when it is one. */
val JsonElement.flag: Boolean? get() = (this as? JsonPrimitive)?.takeIf { !it.isString }?.booleanOrNull

/** A value's number, when it is one. */
val JsonElement.number: Double? get() = (this as? JsonPrimitive)?.takeIf { !it.isString }?.doubleOrNull

/** A JSON value of what Kotlin has: strings, numbers, booleans, lists, maps and nulls. */
fun jsonOf(value: Any?): JsonElement = when (value) {
    null -> JsonNull
    is JsonElement -> value
    is String -> JsonPrimitive(value)
    is Boolean -> JsonPrimitive(value)
    is Number -> JsonPrimitive(value)
    is Map<*, *> -> JsonObject(value.entries.associate { (key, item) -> key.toString() to jsonOf(item) })
    is Iterable<*> -> JsonArray(value.map(::jsonOf))
    else -> JsonPrimitive(value.toString())
}

/** An object of the pairs: `json("type" to "act", "threads" to ids)`. A null value is left out. */
fun json(vararg pairs: Pair<String, Any?>): JSON =
    JsonObject(pairs.filter { it.second != null }.associate { (key, value) -> key to jsonOf(value) })
