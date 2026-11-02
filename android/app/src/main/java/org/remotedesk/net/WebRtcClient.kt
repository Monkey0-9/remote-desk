package org.remotedesk.net

import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Connection states for remote desktop session lifecycle.
 */
enum class ConnectionState {
    IDLE,
    CONNECTING,
    HANDSHAKING,
    ACTIVE,
    DISCONNECTED,
    ERROR
}

/**
 * Normative Wire Protocol Serializer & Session Client for Android Controller.
 *
 * Implements the exact 16-byte framing format defined by remote-desk-proto:
 * - 2 bytes: Magic [0x52, 0x44] ('RD')
 * - 1 byte:  Version (0x01)
 * - 1 byte:  MessageType (u8)
 * - 8 bytes: Sequence Number (u64 Big-Endian)
 * - 4 bytes: Payload Length (u32 Big-Endian)
 * - N bytes: Binary Payload (Little-Endian bincode canonical layout)
 */
class WebRtcClient(
    val deviceId: String,
    val targetHostId: String,
    val hostIp: String,
    val hostPort: Int = 4443,
    private val scope: CoroutineScope = CoroutineScope(Dispatchers.IO),
    var transportSender: ((ByteArray) -> Unit)? = null
) {
    companion object {
        private const val TAG = "RemoteDeskClient"
        const val HEADER_SIZE = 16
        val MAGIC_BYTES = byteArrayOf(0x52, 0x44) // 'RD'
        const val PROTOCOL_VERSION: Byte = 1

        const val MSG_MOUSE_MOVE_ABS: Byte = 0x01
        const val MSG_MOUSE_MOVE_REL: Byte = 0x02
        const val MSG_MOUSE_BUTTON: Byte = 0x03
        const val MSG_MOUSE_WHEEL: Byte = 0x04
        const val MSG_KEYBOARD_KEY: Byte = 0x05
        const val MSG_PING: Byte = 0x20
        const val MSG_PONG: Byte = 0x21
        const val MSG_DISCONNECT: Byte = 0xFF.toByte()
    }

    private val _connectionState = MutableStateFlow(ConnectionState.IDLE)
    val connectionState: StateFlow<ConnectionState> = _connectionState.asStateFlow()

    private val _rttMs = MutableStateFlow(0L)
    val rttMs: StateFlow<Long> = _rttMs.asStateFlow()

    private var sequenceNumber: Long = 0L

    /**
     * Packs a message payload into remote-desk-proto binary frame.
     */
    fun packFrame(msgType: Byte, payload: ByteArray): ByteArray {
        val seq = ++sequenceNumber
        val buffer = ByteBuffer.allocate(HEADER_SIZE + payload.size)
        
        // Header in Big-Endian network order
        buffer.order(ByteOrder.BIG_ENDIAN)
        buffer.put(MAGIC_BYTES)
        buffer.put(PROTOCOL_VERSION)
        buffer.put(msgType)
        buffer.putLong(seq)
        buffer.putInt(payload.size)

        // Payload
        buffer.put(payload)
        return buffer.array()
    }

    /**
     * Builds canonical MouseMoveAbs payload:
     * - variant tag: 0 (Int32 LE)
     * - monitor_id: u32 (Int32 LE)
     * - x: float (Float32 LE)
     * - y: float (Float32 LE)
     */
    fun buildMouseMoveAbsPayload(monitorId: Int, normX: Float, normY: Float): ByteArray {
        val clampedX = normX.coerceIn(0.0f, 1.0f)
        val clampedY = normY.coerceIn(0.0f, 1.0f)

        val buf = ByteBuffer.allocate(16).order(ByteOrder.LITTLE_ENDIAN)
        buf.putInt(0) // Variant tag 0
        buf.putInt(monitorId)
        buf.putFloat(clampedX)
        buf.putFloat(clampedY)
        return buf.array()
    }

    /**
     * Builds canonical MouseButton payload:
     * - variant tag: 2 (Int32 LE)
     * - button: u32 (Int32 LE: 0=Left, 1=Right, 2=Middle, 3=Back, 4=Forward)
     * - state: u32 (Int32 LE: 0=Released, 1=Pressed)
     */
    fun buildMouseButtonPayload(button: Int, down: Boolean): ByteArray {
        val buf = ByteBuffer.allocate(12).order(ByteOrder.LITTLE_ENDIAN)
        buf.putInt(2) // Variant tag 2
        buf.putInt(button)
        buf.putInt(if (down) 1 else 0)
        return buf.array()
    }

    /**
     * Builds canonical MouseWheel payload:
     * - variant tag: 3 (Int32 LE)
     * - delta_x: i16 (Short16 LE)
     * - delta_y: i16 (Short16 LE)
     */
    fun buildMouseWheelPayload(deltaX: Short, deltaY: Short): ByteArray {
        val buf = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN)
        buf.putInt(3) // Variant tag 3
        buf.putShort(deltaX)
        buf.putShort(deltaY)
        return buf.array()
    }

    /**
     * Builds canonical KeyboardKey payload:
     * - variant tag: 4 (Int32 LE)
     * - scancode: u32 (Int32 LE)
     * - state: u32 (Int32 LE: 0=Released, 1=Pressed)
     * - modifiers: u32 (Int32 LE)
     */
    fun buildKeyboardKeyPayload(scancode: Int, down: Boolean, modifiers: Int = 0): ByteArray {
        val buf = ByteBuffer.allocate(16).order(ByteOrder.LITTLE_ENDIAN)
        buf.putInt(4) // Variant tag 4
        buf.putInt(scancode)
        buf.putInt(if (down) 1 else 0)
        buf.putInt(modifiers)
        return buf.array()
    }

    /**
     * Dispatches normalized pointer movement (0.0 .. 1.0).
     */
    fun sendPointerMove(normX: Float, normY: Float, monitorId: Int = 0) {
        val payload = buildMouseMoveAbsPayload(monitorId, normX, normY)
        val frame = packFrame(MSG_MOUSE_MOVE_ABS, payload)
        transportSender?.invoke(frame)
        Log.v(TAG, "Sent MouseMove: ($normX, $normY), frame_len=${frame.size}")
    }

    /**
     * Dispatches mouse button event.
     */
    fun sendMouseButton(buttonCode: Int, down: Boolean) {
        val payload = buildMouseButtonPayload(buttonCode, down)
        val frame = packFrame(MSG_MOUSE_BUTTON, payload)
        transportSender?.invoke(frame)
        Log.d(TAG, "Sent MouseButton: btn=$buttonCode, down=$down, frame_len=${frame.size}")
    }

    /**
     * Dispatches mouse wheel delta.
     */
    fun sendMouseWheel(deltaX: Short, deltaY: Short) {
        val payload = buildMouseWheelPayload(deltaX, deltaY)
        val frame = packFrame(MSG_MOUSE_WHEEL, payload)
        transportSender?.invoke(frame)
        Log.d(TAG, "Sent MouseWheel: dx=$deltaX, dy=$deltaY, frame_len=${frame.size}")
    }

    /**
     * Dispatches keyboard key event.
     */
    fun sendKeyboardKey(scancode: Int, down: Boolean, modifiers: Int = 0) {
        val payload = buildKeyboardKeyPayload(scancode, down, modifiers)
        val frame = packFrame(MSG_KEYBOARD_KEY, payload)
        transportSender?.invoke(frame)
        Log.d(TAG, "Sent KeyboardKey: scancode=$scancode, down=$down, frame_len=${frame.size}")
    }

    /**
     * Disconnects the session.
     */
    fun disconnect() {
        val buf = ByteBuffer.allocate(16).order(ByteOrder.LITTLE_ENDIAN)
        buf.putInt(12) // Variant tag 12 (Disconnect)
        buf.putInt(0)  // Reason code 0
        buf.putLong(0) // Reason string len 0
        val frame = packFrame(MSG_DISCONNECT, buf.array())
        transportSender?.invoke(frame)
        Log.i(TAG, "Sent Disconnect frame (${frame.size} bytes). Closing connection.")
        _connectionState.value = ConnectionState.DISCONNECTED
    }
}
