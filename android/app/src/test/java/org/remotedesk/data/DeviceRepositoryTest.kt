package org.remotedesk.data

import org.junit.Assert.*
import org.junit.Test
import java.io.File

class DeviceRepositoryTest {

    @Test
    fun testAddGetAndRemoveDevice() {
        val tempFile = File.createTempFile("devices_test", ".txt")
        tempFile.deleteOnExit()

        val repo = PersistentDeviceRepository(tempFile)
        assertEquals(0, repo.getAllDevices().size)

        val dev = SavedDevice(
            id = "rd-1234-5678",
            name = "Test Host",
            osBadge = "Linux (Wayland)",
            address = "192.168.1.100",
            isOnline = true
        )
        repo.addOrUpdateDevice(dev)

        assertEquals(1, repo.getAllDevices().size)
        val fetched = repo.getDevice("rd-1234-5678")
        assertNotNull(fetched)
        assertEquals("Test Host", fetched?.name)

        // Test persistence reload
        val repo2 = PersistentDeviceRepository(tempFile)
        assertEquals(1, repo2.getAllDevices().size)
        assertEquals("Test Host", repo2.getDevice("rd-1234-5678")?.name)

        // Remove device
        val removed = repo2.removeDevice("rd-1234-5678")
        assertTrue(removed)
        assertEquals(0, repo2.getAllDevices().size)

        tempFile.delete()
    }

    @Test
    fun testUpdateOnlineAndTrustStatus() {
        val tempFile = File.createTempFile("devices_status_test", ".txt")
        tempFile.deleteOnExit()

        val repo = PersistentDeviceRepository(tempFile)
        val dev = SavedDevice(
            id = "rd-9999-0000",
            name = "Windows 11 PC",
            osBadge = "Windows 11",
            address = "10.0.0.12",
            isOnline = false,
            isTrusted = false
        )
        repo.addOrUpdateDevice(dev)

        repo.setDeviceOnline("rd-9999-0000", true)
        assertTrue(repo.getDevice("rd-9999-0000")?.isOnline == true)

        repo.setDeviceTrusted("rd-9999-0000", true)
        assertTrue(repo.getDevice("rd-9999-0000")?.isTrusted == true)

        tempFile.delete()
    }
}
