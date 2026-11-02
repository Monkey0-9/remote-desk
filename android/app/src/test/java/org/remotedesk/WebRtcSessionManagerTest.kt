package org.remotedesk

import org.junit.Assert.*
import org.junit.Test
import org.remotedesk.net.ConnectionState
import org.remotedesk.net.WebRtcClient
import org.remotedesk.net.WebRtcSessionManager
import java.nio.ByteBuffer
import java.nio.ByteOrder

class WebRtcSessionManagerTest {

    @Test
    fun testWebRtcClientPacksNormative16ByteHeader() {
        var lastSentFrame: ByteArray? = null
        val client = WebRtcClient(
            deviceId = "rd-test-client",
            targetHostId = "rd-test-host",
            hostIp = "127.0.0.1",
            transportSender = { frame ->
                lastSentFrame = frame
            }
        )

        // Dispatch pointer move
        client.sendPointerMove(0.5f, 0.25f, monitorId = 0)

        assertNotNull("Frame must be delivered to transport sender", lastSentFrame)
        val frame = lastSentFrame!!
        assertTrue("Frame must be at least header size (16 bytes)", frame.size >= 16)

        val buf = ByteBuffer.wrap(frame).order(ByteOrder.BIG_ENDIAN)
        val magic0 = buf.get()
        val magic1 = buf.get()
        assertEquals(0x52.toByte(), magic0) // 'R'
        assertEquals(0x44.toByte(), magic1) // 'D'

        val version = buf.get()
        assertEquals(1.toByte(), version)

        val msgType = buf.get()
        assertEquals(WebRtcClient.MSG_MOUSE_MOVE_ABS, msgType)

        val seq = buf.getLong()
        assertEquals(1L, seq)

        val payloadLen = buf.getInt()
        assertEquals(16, payloadLen) // 4B variant + 4B monitor + 4B x + 4B y

        // Verify little-endian payload
        buf.order(ByteOrder.LITTLE_ENDIAN)
        val variantTag = buf.getInt()
        assertEquals(0, variantTag)
        val monitorId = buf.getInt()
        assertEquals(0, monitorId)
        val x = buf.getFloat()
        val y = buf.getFloat()
        assertEquals(0.5f, x, 0.001f)
        assertEquals(0.25f, y, 0.001f)
    }

    @Test
    fun testWebRtcSessionManagerLifecycleTransitions() {
        val manager = WebRtcSessionManager(
            deviceId = "rd-1234-5678",
            targetHostId = "rd-9999-8888",
            hostAddress = "127.0.0.1",
            hostPort = 4443
        )

        assertEquals(ConnectionState.IDLE, manager.connectionState.value)
        assertNull(manager.sasCode.value)

        // Disconnecting from IDLE transitions cleanly to DISCONNECTED
        manager.disconnect()
        assertEquals(ConnectionState.DISCONNECTED, manager.connectionState.value)
    }
}
