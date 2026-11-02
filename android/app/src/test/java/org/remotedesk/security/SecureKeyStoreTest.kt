package org.remotedesk.security

import org.junit.Assert.*
import org.junit.Test

class SecureKeyStoreTest {

    @Test
    fun testKeyStoreGenerationAndRevocation() {
        val keyStore = SecureKeyStore()
        val seed = keyStore.getLocalDeviceSeed()

        assertEquals(32, seed.size)
        assertFalse(seed.all { it == 0.toByte() })

        // Trust host device
        val hostId = "rd-1234-5678"
        val hostKeyHex = "9f82ab73645c"
        keyStore.trustDevice(hostId, hostKeyHex)

        assertTrue(keyStore.isDeviceTrusted(hostId))
        assertEquals(1, keyStore.listTrustedDevices().size)

        // Revoke host device
        val revoked = keyStore.revokeDevice(hostId)
        assertTrue(revoked)
        assertFalse(keyStore.isDeviceTrusted(hostId))
        assertEquals(0, keyStore.listTrustedDevices().size)
    }

    @Test
    fun testClearAllKeysWipesMemory() {
        val keyStore = SecureKeyStore()
        keyStore.trustDevice("rd-9999-0000", "deadbeef")

        keyStore.clearAllKeys()

        assertEquals(0, keyStore.listTrustedDevices().size)
        val wipedSeed = keyStore.getLocalDeviceSeed()
        assertTrue(wipedSeed.all { it == 0.toByte() })
    }

    @Test
    fun testEncryptedPersistenceAndReload() {
        val tempFile = java.io.File.createTempFile("rd_keystore_test", ".dat")
        try {
            val keyStore1 = SecureKeyStore(tempFile)
            val seed1 = keyStore1.getLocalDeviceSeed()
            keyStore1.trustDevice("rd-host-alpha", "11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff")

            val fileBytes = tempFile.readBytes()
            // Verify file starts with SKS\1 magic
            assertEquals(0x53.toByte(), fileBytes[0]) // 'S'
            assertEquals(0x4B.toByte(), fileBytes[1]) // 'K'
            assertEquals(0x53.toByte(), fileBytes[2]) // 'S'
            assertEquals(0x01.toByte(), fileBytes[3]) // 1

            // Verify plaintext seed and device ID are NOT directly visible in raw file bytes
            val fileContentString = String(fileBytes, Charsets.ISO_8859_1)
            assertFalse(fileContentString.contains("rd-host-alpha"))

            // Reload into fresh instance
            val keyStore2 = SecureKeyStore(tempFile)
            assertArrayEquals(seed1, keyStore2.getLocalDeviceSeed())
            assertTrue(keyStore2.isDeviceTrusted("rd-host-alpha"))
        } finally {
            tempFile.delete()
        }
    }
}
