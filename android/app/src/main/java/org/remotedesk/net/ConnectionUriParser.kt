package org.remotedesk.net

import org.json.JSONObject
import java.net.URLDecoder

data class ParsedConnectionTarget(
    val host: String,
    val port: Int,
    val isSignaling: Boolean,
    val targetHostId: String,
    val friendlyName: String,
    val rawCleanAddress: String
)

object ConnectionUriParser {

    /**
     * Parses any format of scanned QR code or user input into a validated connection endpoint.
     * Supported formats:
     * - remotedesk://192.168.1.11:4443?id=rd-8623-8185&name=WorkPC
     * - remotedesk://192.168.1.11:4443
     * - http://192.168.1.11:4443/
     * - ws://127.0.0.1:4443
     * - signaling:rendezvous
     * - 192.168.1.11:4443
     * - 192.168.1.11
     * - rd-8623-8185
     * - JSON QR payload: {"scheme":"remotedesk","id":"rd-8623-8185","ip":"192.168.1.11","port":4443}
     */
    fun parse(rawInput: String, defaultId: String = ""): ParsedConnectionTarget {
        var input = rawInput.trim()
        var targetId = defaultId
        var friendlyName = ""

        // Check for JSON payload
        if (input.startsWith("{") && input.endsWith("}")) {
            try {
                val json = JSONObject(input)
                if (json.has("id")) targetId = json.getString("id")
                if (json.has("name")) friendlyName = json.getString("name")
                val ip = when {
                    json.has("ip") -> json.getString("ip")
                    json.has("addr") -> json.getString("addr")
                    json.has("address") -> json.getString("address")
                    else -> "127.0.0.1"
                }
                val port = json.optInt("port", 4443)
                val finalId = if (targetId.isNotBlank()) targetId else ip
                val name = if (friendlyName.isNotBlank()) friendlyName else finalId
                return ParsedConnectionTarget(
                    host = ip,
                    port = port,
                    isSignaling = false,
                    targetHostId = finalId,
                    friendlyName = name,
                    rawCleanAddress = "$ip:$port"
                )
            } catch (_: Exception) {}
        }

        // Strip custom scheme
        if (input.startsWith("remotedesk://", ignoreCase = true)) {
            input = input.substring("remotedesk://".length)
        } else if (input.startsWith("http://", ignoreCase = true)) {
            input = input.substring("http://".length)
        } else if (input.startsWith("https://", ignoreCase = true)) {
            input = input.substring("https://".length)
        }

        // Parse query params if present (e.g. ?id=rd-8623-8185&name=WorkPC)
        if (input.contains("?")) {
            val query = input.substringAfter("?")
            input = input.substringBefore("?")
            for (part in query.split("&")) {
                val kv = part.split("=")
                if (kv.size == 2) {
                    val key = kv[0].trim()
                    val value = try { URLDecoder.decode(kv[1].trim(), "UTF-8") } catch (_: Exception) { kv[1].trim() }
                    if (key.equals("id", ignoreCase = true)) {
                        targetId = value
                    } else if (key.equals("name", ignoreCase = true)) {
                        friendlyName = value
                    }
                }
            }
        }

        // Strip trailing slash
        if (input.contains("/")) {
            input = input.substringBefore("/")
        }

        // Check if signaling URL
        val isSignaling = input.startsWith("ws://", ignoreCase = true) ||
                          input.startsWith("wss://", ignoreCase = true) ||
                          input.equals("signaling:rendezvous", ignoreCase = true)

        if (isSignaling) {
            val finalId = if (targetId.isNotBlank()) targetId else defaultId
            val name = if (friendlyName.isNotBlank()) friendlyName else finalId
            return ParsedConnectionTarget(
                host = input,
                port = 4443,
                isSignaling = true,
                targetHostId = finalId,
                friendlyName = name,
                rawCleanAddress = input
            )
        }

        // Check if device ID without host (e.g. rd-8623-8185)
        val isDeviceIdOnly = input.matches(Regex("^[a-zA-Z0-9]+-[0-9]{4}-[0-9]{4}$")) ||
                            (input.startsWith("rd-", ignoreCase = true) && !input.contains(".") && !input.contains(":"))

        if (isDeviceIdOnly) {
            val finalId = input
            val name = if (friendlyName.isNotBlank()) friendlyName else finalId
            return ParsedConnectionTarget(
                host = "127.0.0.1",
                port = 4443,
                isSignaling = true,
                targetHostId = finalId,
                friendlyName = name,
                rawCleanAddress = "signaling:rendezvous"
            )
        }

        // Standard host[:port]
        val host = if (input.contains(":")) input.substringBefore(":") else input
        val port = if (input.contains(":")) input.substringAfter(":").toIntOrNull() ?: 4443 else 4443
        val finalId = if (targetId.isNotBlank()) targetId else input
        val name = if (friendlyName.isNotBlank()) friendlyName else finalId

        return ParsedConnectionTarget(
            host = host,
            port = port,
            isSignaling = false,
            targetHostId = finalId,
            friendlyName = name,
            rawCleanAddress = "$host:$port"
        )
    }
}
