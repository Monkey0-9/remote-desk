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
}
