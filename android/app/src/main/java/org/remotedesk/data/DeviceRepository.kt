package org.remotedesk.data

import java.io.File

/**
 * Saved device record representing a paired or discovered Remote-Desk host
 */
data class SavedDevice(
    val id: String,
    val name: String,
    val osBadge: String,
    val address: String,
    val isOnline: Boolean,
    val lastSeenTimestamp: Long = System.currentTimeMillis(),
    val isTrusted: Boolean = false,
    val fingerprintHex: String = ""
)

/**
 * Device repository interface for listing, updating, and revoking remote hosts
 */
interface DeviceRepository {
    fun getAllDevices(): List<SavedDevice>
    fun getDevice(id: String): SavedDevice?
    fun addOrUpdateDevice(device: SavedDevice)
    fun removeDevice(id: String): Boolean
    fun setDeviceOnline(id: String, online: Boolean)
    fun setDeviceTrusted(id: String, trusted: Boolean)
}

/**
 * Persistent device repository storing saved hosts in a structured format
 */
class PersistentDeviceRepository(private val storageFile: File? = null) : DeviceRepository {
    private val devices = mutableMapOf<String, SavedDevice>()

    init {
        load()
    }

    override fun getAllDevices(): List<SavedDevice> = devices.values.toList()

    override fun getDevice(id: String): SavedDevice? = devices[id]

    override fun addOrUpdateDevice(device: SavedDevice) {
        devices[device.id] = device
        save()
    }

    override fun removeDevice(id: String): Boolean {
        val removed = devices.remove(id) != null
        if (removed) save()
        return removed
    }

    override fun setDeviceOnline(id: String, online: Boolean) {
        val current = devices[id]
        if (current != null) {
            devices[id] = current.copy(
                isOnline = online,
                lastSeenTimestamp = System.currentTimeMillis()
            )
            save()
        }
    }

    override fun setDeviceTrusted(id: String, trusted: Boolean) {
        val current = devices[id]
        if (current != null) {
            devices[id] = current.copy(isTrusted = trusted)
            save()
        }
    }

    private fun load() {
        if (storageFile != null && storageFile.exists()) {
            try {
                val lines = storageFile.readLines()
                for (line in lines) {
                    val parts = line.split("|")
                    if (parts.size >= 5) {
                        val dev = SavedDevice(
                            id = parts[0],
                            name = parts[1],
                            osBadge = parts[2],
                            address = parts[3],
                            isOnline = parts[4].toBoolean(),
                            lastSeenTimestamp = parts.getOrNull(5)?.toLongOrNull() ?: 0L,
                            isTrusted = parts.getOrNull(6)?.toBoolean() ?: false,
                            fingerprintHex = parts.getOrNull(7) ?: ""
                        )
                        devices[dev.id] = dev
                    }
                }
            } catch (_: Exception) {
                // Graceful fallback on corrupt storage
            }
        }
    }

    private fun save() {
        if (storageFile != null) {
            try {
                val parent = storageFile.parentFile
                if (parent != null && !parent.exists()) {
                    parent.mkdirs()
                }
                val content = devices.values.joinToString("\n") {
                    "${it.id}|${it.name}|${it.osBadge}|${it.address}|${it.isOnline}|${it.lastSeenTimestamp}|${it.isTrusted}|${it.fingerprintHex}"
                }
                storageFile.writeText(content)
            } catch (_: Exception) {
                // Silently swallow in read-only sandbox
            }
        }
    }
}
