package org.remotedesk

import org.junit.Assert.assertEquals
import org.junit.Test
import org.remotedesk.net.WebRtcClient
import java.nio.ByteBuffer
import java.nio.ByteOrder

class WireProtocolGoldenTest {

    @Test
    fun testMouseMoveAbsPayloadMatchesRustWireLayout() {
        val client = WebRtcClient("test-device", "target-host", "127.0.0.1", 4443)
        val payload = client.buildMouseMoveAbsPayload(0, 0.5f, 0.25f)

        assertEquals(16, payload.size)
        val buf = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)
        assertEquals(0, buf.getInt()) // Variant tag 0
        assertEquals(0, buf.getInt()) // Monitor ID 0
        assertEquals(0.5f, buf.getFloat(), 0.0001f) // X
        assertEquals(0.25f, buf.getFloat(), 0.0001f) // Y

        val frame = client.packFrame(WebRtcClient.MSG_MOUSE_MOVE_ABS, payload)
        assertEquals(32, frame.size) // 16 header + 16 payload
        assertEquals('R'.code.toByte(), frame[0])
        assertEquals('D'.code.toByte(), frame[1])
        assertEquals(1.toByte(), frame[2]) // Version 1
        assertEquals(WebRtcClient.MSG_MOUSE_MOVE_ABS, frame[3])
    }

    @Test
    fun testMouseButtonPayloadMatchesRustWireLayout() {
        val client = WebRtcClient("test-device", "target-host", "127.0.0.1", 4443)
        val payload = client.buildMouseButtonPayload(0, true) // Left, Pressed

        assertEquals(12, payload.size)
        val buf = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)
        assertEquals(2, buf.getInt()) // Variant tag 2
        assertEquals(0, buf.getInt()) // Left button = 0
        assertEquals(1, buf.getInt()) // Pressed = 1

        val frame = client.packFrame(WebRtcClient.MSG_MOUSE_BUTTON, payload)
        assertEquals(28, frame.size) // 16 header + 12 payload
    }

    @Test
    fun testKeyboardKeyPayloadMatchesRustWireLayout() {
        val client = WebRtcClient("test-device", "target-host", "127.0.0.1", 4443)
        val payload = client.buildKeyboardKeyPayload(30, true, 1) // KEY_A, Pressed, Shift

        assertEquals(16, payload.size)
        val buf = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)
        assertEquals(4, buf.getInt()) // Variant tag 4
        assertEquals(30, buf.getInt()) // Scancode 30
        assertEquals(1, buf.getInt()) // Pressed = 1
        assertEquals(1, buf.getInt()) // Modifiers = 1

        val frame = client.packFrame(WebRtcClient.MSG_KEYBOARD_KEY, payload)
        assertEquals(32, frame.size) // 16 header + 16 payload
    }

    @Test
    fun testMouseWheelPayloadMatchesRustWireLayout() {
        val client = WebRtcClient("test-device", "target-host", "127.0.0.1", 4443)
        val payload = client.buildMouseWheelPayload(10.toShort(), (-120).toShort())

        assertEquals(8, payload.size)
        val buf = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)
        assertEquals(3, buf.getInt()) // Variant tag 3
        assertEquals(10.toShort(), buf.getShort())
        assertEquals((-120).toShort(), buf.getShort())

        val frame = client.packFrame(WebRtcClient.MSG_MOUSE_WHEEL, payload)
        assertEquals(24, frame.size) // 16 header + 8 payload
        assertEquals(WebRtcClient.MSG_MOUSE_WHEEL, frame[3])
    }

    @Test
    fun testDisconnectPayloadMatchesRustWireLayout() {
        val client = WebRtcClient("test-device", "target-host", "127.0.0.1", 4443)
        val buf = ByteBuffer.allocate(16).order(ByteOrder.LITTLE_ENDIAN)
        buf.putInt(12) // Variant tag 12 (Disconnect)
        buf.putInt(0)  // Reason code 0
        buf.putLong(0) // Reason string len 0
        val payload = buf.array()

        val frame = client.packFrame(WebRtcClient.MSG_DISCONNECT, payload)
        assertEquals(32, frame.size)
        assertEquals(WebRtcClient.MSG_DISCONNECT, frame[3])
    }
}
