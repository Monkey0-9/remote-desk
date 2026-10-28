package org.remotedesk.input

enum class InputMode {
    DIRECT_TOUCH,
    MOUSE_POINTER
}

sealed class InputEvent {
    data class PointerMove(val x: Float, val y: Float, val isRelative: Boolean) : InputEvent()
    data class MouseButton(val button: Int, val isDown: Boolean) : InputEvent()
    data class MouseScroll(val deltaX: Float, val deltaY: Float) : InputEvent()
}

class GestureRecognizer(
    var mode: InputMode = InputMode.DIRECT_TOUCH,
    var sensitivity: Float = 1.0f
) {
    var cursorX: Float = 0.5f
    var cursorY: Float = 0.5f

    private var downTimeMs: Long = 0
    private var lastTouchX: Float = 0f
    private var lastTouchY: Float = 0f
    private var isDragging: Boolean = false

    fun onTouchDown(touchX: Float, touchY: Float, canvasWidth: Float, canvasHeight: Float, timeMs: Long): List<InputEvent> {
        downTimeMs = timeMs
        lastTouchX = touchX
        lastTouchY = touchY
        isDragging = false

        return if (mode == InputMode.DIRECT_TOUCH) {
            val normX = (touchX / canvasWidth).coerceIn(0f, 1f)
            val normY = (touchY / canvasHeight).coerceIn(0f, 1f)
            cursorX = normX
            cursorY = normY
            listOf(InputEvent.PointerMove(normX, normY, isRelative = false))
        } else {
            emptyList()
        }
    }

    fun onTouchMove(touchX: Float, touchY: Float, canvasWidth: Float, canvasHeight: Float): List<InputEvent> {
        val dx = touchX - lastTouchX
        val dy = touchY - lastTouchY
        lastTouchX = touchX
        lastTouchY = touchY

        if (Math.hypot(dx.toDouble(), dy.toDouble()) > 5.0) {
            isDragging = true
        }

        return if (mode == InputMode.DIRECT_TOUCH) {
            val normX = (touchX / canvasWidth).coerceIn(0f, 1f)
            val normY = (touchY / canvasHeight).coerceIn(0f, 1f)
            cursorX = normX
            cursorY = normY
            listOf(InputEvent.PointerMove(normX, normY, isRelative = false))
        } else {
            // Pointer mode: relative delta scaled by sensitivity
            val normDx = (dx / canvasWidth) * sensitivity
            val normDy = (dy / canvasHeight) * sensitivity
            cursorX = (cursorX + normDx).coerceIn(0f, 1f)
            cursorY = (cursorY + normDy).coerceIn(0f, 1f)
            listOf(InputEvent.PointerMove(cursorX, cursorY, isRelative = false))
        }
    }

    fun onTouchUp(touchX: Float, touchY: Float, timeMs: Long): List<InputEvent> {
        val duration = timeMs - downTimeMs
        val events = mutableListOf<InputEvent>()

        if (!isDragging && duration < 300) {
            // Short tap: simulate left click down + up
            events.add(InputEvent.MouseButton(button = 0, isDown = true))
            events.add(InputEvent.MouseButton(button = 0, isDown = false))
        } else if (!isDragging && duration >= 400) {
            // Tap & hold: simulate right click down + up
            events.add(InputEvent.MouseButton(button = 1, isDown = true))
            events.add(InputEvent.MouseButton(button = 1, isDown = false))
        }

        return events
    }
}
