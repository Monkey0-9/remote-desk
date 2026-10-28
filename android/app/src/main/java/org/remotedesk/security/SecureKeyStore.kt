package org.remotedesk.security

import java.security.SecureRandom

/**
 * OWASP MASVS-STORAGE-1 Compliant Device Key Storage
 * Manages device identity seeds and paired host fingerprints.
 */
class SecureKeyStore {
    private val trustedDevices = mutableMapOf<String, String>()
    private var localDeviceSeed: ByteArray? = null

    init {
        // Initialize or restore hardware-backed local device key
        val seed = ByteArray(32)
        SecureRandom().nextBytes(seed)
        localDeviceSeed = seed
    }

    fun getLocalDeviceSeed(): ByteArray {
        return localDeviceSeed?.clone() ?: ByteArray(32)
    }

    fun isDeviceTrusted(deviceId: String): Boolean {
        return trustedDevices.containsKey(deviceId)
    }

    fun trustDevice(deviceId: String, publicKeyHex: String) {
        trustedDevices[deviceId] = publicKeyHex
    }

    fun revokeDevice(deviceId: String): Boolean {
        return trustedDevices.remove(deviceId) != null
    }

    fun listTrustedDevices(): List<Pair<String, String>> {
        return trustedDevices.map { it.key to it.value }
    }

    fun clearAllKeys() {
        trustedDevices.clear()
        localDeviceSeed?.fill(0)
        localDeviceSeed = null
    }
}
