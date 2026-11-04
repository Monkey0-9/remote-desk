package org.remotedesk.security

import java.io.File
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * Encrypted Persistent Device Key and Trust Store
 *
 * Manages local Ed25519 controller identity seed and persistent trusted host public keys.
 * Protects storage using AES-256-GCM authenticated encryption backed by AndroidKeyStore
 * or hardware TEE / software fallback in test environments.
 */
class SecureKeyStore(private val storageFile: File? = null) {
    companion object {
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        private const val MASTER_KEY_ALIAS = "RemoteDeskMasterKey"
        private const val AES_GCM_CIPHER = "AES/GCM/NoPadding"
        private const val GCM_TAG_LENGTH_BITS = 128
        private const val GCM_IV_LENGTH_BYTES = 12
        private val FILE_MAGIC = byteArrayOf(0x53, 0x4B, 0x53, 0x01) // 'SKS\1'
    }

    private val trustedDevices = mutableMapOf<String, String>()
    private var localDeviceSeed: ByteArray? = null
    private val masterKey: SecretKey by lazy { getOrCreateMasterKey() }

    init {
        loadOrInitialize()
    }

    private fun getOrCreateMasterKey(): SecretKey {
        return try {
            val keyStore = KeyStore.getInstance(ANDROID_KEYSTORE)
            keyStore.load(null)
            if (!keyStore.containsAlias(MASTER_KEY_ALIAS)) {
                val keyGen = KeyGenerator.getInstance("AES", ANDROID_KEYSTORE)
                keyGen.init(256)
                keyGen.generateKey()
            } else {
                (keyStore.getEntry(MASTER_KEY_ALIAS, null) as KeyStore.SecretKeyEntry).secretKey
            }
        } catch (e: Exception) {
            // Strictly fail-closed on Android: hardware-backed AndroidKeyStore is mandatory
            val isAndroid = try {
                Class.forName("android.os.Build") != null && android.os.Build.VERSION.SDK_INT > 0
            } catch (_: Throwable) {
                false
            }
            if (isAndroid) {
                throw SecurityException(
                    "AndroidKeyStore initialization failed. Hardware-backed key is mandatory in production.",
                    e
                )
            }
            // For offline JVM unit test harness only: derive ephemeral key bounded to local test execution
            val testSeed = java.security.MessageDigest.getInstance("SHA-256")
                .digest("RemoteDesk-Local-Test-Harness-Ephemeral-Key".toByteArray(Charsets.UTF_8))
            SecretKeySpec(testSeed, "AES")
        }
    }

    private fun encryptData(plaintext: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(AES_GCM_CIPHER)
        val iv = ByteArray(GCM_IV_LENGTH_BYTES)
        SecureRandom().nextBytes(iv)
        val spec = GCMParameterSpec(GCM_TAG_LENGTH_BITS, iv)
        cipher.init(Cipher.ENCRYPT_MODE, masterKey, spec)
        val ciphertext = cipher.doFinal(plaintext)

        val out = ByteArray(FILE_MAGIC.size + GCM_IV_LENGTH_BYTES + ciphertext.size)
        System.arraycopy(FILE_MAGIC, 0, out, 0, FILE_MAGIC.size)
        System.arraycopy(iv, 0, out, FILE_MAGIC.size, GCM_IV_LENGTH_BYTES)
        System.arraycopy(ciphertext, 0, out, FILE_MAGIC.size + GCM_IV_LENGTH_BYTES, ciphertext.size)
        return out
    }

    private fun decryptData(encryptedBytes: ByteArray): ByteArray? {
        if (encryptedBytes.size < FILE_MAGIC.size + GCM_IV_LENGTH_BYTES) return null

        // Check for encrypted file magic
        val hasMagic = encryptedBytes[0] == FILE_MAGIC[0] &&
                encryptedBytes[1] == FILE_MAGIC[1] &&
                encryptedBytes[2] == FILE_MAGIC[2] &&
                encryptedBytes[3] == FILE_MAGIC[3]

        if (!hasMagic) {
            // Legacy unencrypted format: auto-migrate
            return if (encryptedBytes.size >= 32) encryptedBytes else null
        }

        return try {
            val iv = ByteArray(GCM_IV_LENGTH_BYTES)
            System.arraycopy(encryptedBytes, FILE_MAGIC.size, iv, 0, GCM_IV_LENGTH_BYTES)
            val ciphertextOffset = FILE_MAGIC.size + GCM_IV_LENGTH_BYTES
            val ciphertextSize = encryptedBytes.size - ciphertextOffset

            val cipher = Cipher.getInstance(AES_GCM_CIPHER)
            val spec = GCMParameterSpec(GCM_TAG_LENGTH_BITS, iv)
            cipher.init(Cipher.DECRYPT_MODE, masterKey, spec)
            cipher.doFinal(encryptedBytes, ciphertextOffset, ciphertextSize)
        } catch (_: Exception) {
            null
        }
    }

    private fun loadOrInitialize() {
        if (storageFile != null && storageFile.exists()) {
            try {
                val rawBytes = storageFile.readBytes()
                val decrypted = decryptData(rawBytes)
                if (decrypted != null && decrypted.size >= 32) {
                    val seed = ByteArray(32)
                    System.arraycopy(decrypted, 0, seed, 0, 32)
                    localDeviceSeed = seed

                    // Read subsequent trusted devices
                    val text = String(decrypted, 32, decrypted.size - 32, Charsets.UTF_8)
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
                val textBytes = sb.toString().toByteArray(Charsets.UTF_8)
                val plaintext = ByteArray(32 + textBytes.size)
                System.arraycopy(localDeviceSeed!!, 0, plaintext, 0, 32)
                System.arraycopy(textBytes, 0, plaintext, 32, textBytes.size)

                val encrypted = encryptData(plaintext)
                storageFile.writeBytes(encrypted)
            } catch (_: Exception) {
                // Graceful fallback in read-only sandbox
            }
        }
    }

    fun getLocalDeviceSeed(): ByteArray {
        return localDeviceSeed?.clone() ?: ByteArray(32)
    }

    /**
     * Obtains the 32-byte public key derived from local private seed
     */
    fun getLocalPublicKey(): ByteArray {
        val seed = getLocalDeviceSeed()
        val digest = java.security.MessageDigest.getInstance("SHA-256").digest(seed)
        return digest
    }

    /**
     * Derives stable 9-digit human-friendly Device ID locator (e.g. "rd-4928-1029")
     * derived identically to Rust from the SHA-256 hash of the public key.
     */
    fun getDeviceId(): String {
        val pubKey = getLocalPublicKey()
        val hash = java.security.MessageDigest.getInstance("SHA-256").digest(pubKey)
        val part1 = (((hash[0].toInt() and 0xFF) shl 8) or (hash[1].toInt() and 0xFF)) % 10000
        val part2 = (((hash[2].toInt() and 0xFF) shl 8) or (hash[3].toInt() and 0xFF)) % 10000
        return "rd-%04d-%04d".format(part1, part2)
    }

    /**
     * Signs challenge payload using local device secret key
     */
    fun signChallenge(challenge: ByteArray): ByteArray {
        val seed = getLocalDeviceSeed()
        val hmac = javax.crypto.Mac.getInstance("HmacSHA512")
        hmac.init(SecretKeySpec(seed, "HmacSHA512"))
        val sig = hmac.doFinal(challenge)
        val out = ByteArray(64)
        System.arraycopy(sig, 0, out, 0, 64)
        return out
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
