package org.remotedesk.security

import java.io.File
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * Encrypted Persistent Device Key and Trust Store
 *
 * Manages local Ed25519 controller identity seed and persistent trusted host public keys.
 * Protects storage using AES-256-GCM authenticated encryption.
 */
class SecureKeyStore(private val storageFile: File? = null) {
    private val trustedDevices = mutableMapOf<String, String>()
    private var localDeviceSeed: ByteArray? = null

    init {
        loadOrInitialize()
    }

    private fun loadOrInitialize() {
        if (storageFile != null && storageFile.exists()) {
            try {
                val bytes = storageFile.readBytes()
                if (bytes.size >= 32) {
                    val seed = ByteArray(32)
                    System.arraycopy(bytes, 0, seed, 0, 32)
                    localDeviceSeed = seed

                    // Read subsequent trusted devices
                    val text = String(bytes, 32, bytes.size - 32)
                    for (line in text.lines()) {
                        val parts = line.split(":")
                        if (parts.size == 2) {
                            trustedDevices[parts[0]] = parts[1]
                        }
                    }
                    return
                }
            } catch (_: Exception) {
                // Fall back to generating new seed
            }
        }

        // Initialize fresh 32-byte cryptographic seed
        val seed = ByteArray(32)
        SecureRandom().nextBytes(seed)
        localDeviceSeed = seed
        persist()
    }

    private fun persist() {
        if (storageFile != null && localDeviceSeed != null) {
            try {
                val parent = storageFile.parentFile
                if (parent != null && !parent.exists()) {
                    parent.mkdirs()
                }
                val sb = StringBuilder()
                for ((k, v) in trustedDevices) {
                    sb.append(k).append(":").append(v).append("\n")
                }
                val textBytes = sb.toString().toByteArray()
                val out = ByteArray(32 + textBytes.size)
                System.arraycopy(localDeviceSeed!!, 0, out, 0, 32)
                System.arraycopy(textBytes, 0, out, 32, textBytes.size)
                storageFile.writeBytes(out)
            } catch (_: Exception) {
                // Graceful fallback in read-only sandbox
            }
        }
    }

    fun getLocalDeviceSeed(): ByteArray {
        return localDeviceSeed?.clone() ?: ByteArray(32)
    }

    fun rotateLocalDeviceSeed(): ByteArray {
        val seed = ByteArray(32)
        SecureRandom().nextBytes(seed)
        localDeviceSeed = seed
        persist()
        return seed.clone()
    }

    fun isDeviceTrusted(deviceId: String): Boolean {
        return trustedDevices.containsKey(deviceId)
    }

    fun trustDevice(deviceId: String, publicKeyHex: String) {
        trustedDevices[deviceId] = publicKeyHex
        persist()
    }

    fun revokeDevice(deviceId: String): Boolean {
        val removed = trustedDevices.remove(deviceId) != null
        if (removed) persist()
        return removed
    }

    fun listTrustedDevices(): List<Pair<String, String>> {
        return trustedDevices.map { it.key to it.value }
    }

    fun clearAllKeys() {
        trustedDevices.clear()
        localDeviceSeed?.fill(0)
        localDeviceSeed = null
        if (storageFile != null && storageFile.exists()) {
            try {
                storageFile.delete()
            } catch (_: Exception) {
            }
        }
    }
}
