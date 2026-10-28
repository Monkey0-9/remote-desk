package org.remotedesk.input

import org.junit.Assert.*
import org.junit.Test

class GestureRecognizerTest {

    @Test
    fun testDirectTouchMode() {
        val recognizer = GestureRecognizer(mode = InputMode.DIRECT_TOUCH)
        val events = recognizer.onTouchDown(500f, 250f, 1000f, 1000f, 1000L)

        assertEquals(1, events.size)
        val move = events[0] as InputEvent.PointerMove
        assertEquals(0.5f, move.x, 0.001f)
        assertEquals(0.25f, move.y, 0.001f)
        assertFalse(move.isRelative)
    }

    @Test
    fun testMousePointerModeRelativeDelta() {
        val recognizer = GestureRecognizer(mode = InputMode.MOUSE_POINTER, sensitivity = 1.0f)
        recognizer.cursorX = 0.5f
        recognizer.cursorY = 0.5f

        // Touch down at (200, 200)
        recognizer.onTouchDown(200f, 200f, 1000f, 1000f, 1000L)

        // Drag to (300, 250) -> delta = (+100, +50) on 1000x1000 = (+0.1, +0.05)
        val events = recognizer.onTouchMove(300f, 250f, 1000f, 1000f)

        assertEquals(1, events.size)
        val move = events[0] as InputEvent.PointerMove
        assertEquals(0.6f, move.x, 0.001f)
        assertEquals(0.55f, move.y, 0.001f)
    }

    @Test
    fun testShortTapGeneratesLeftClick() {
        val recognizer = GestureRecognizer(mode = InputMode.DIRECT_TOUCH)
        recognizer.onTouchDown(100f, 100f, 1000f, 1000f, 1000L)
        val events = recognizer.onTouchUp(100f, 100f, 1150L) // 150ms tap

        assertEquals(2, events.size)
        val down = events[0] as InputEvent.MouseButton
        val up = events[1] as InputEvent.MouseButton

        assertEquals(0, down.button) // Left click
        assertTrue(down.isDown)

        assertEquals(0, up.button)
        assertFalse(up.isDown)
    }

    @Test
    fun testLongPressGeneratesRightClick() {
        val recognizer = GestureRecognizer(mode = InputMode.DIRECT_TOUCH)
        recognizer.onTouchDown(100f, 100f, 1000f, 1000f, 1000L)
        val events = recognizer.onTouchUp(100f, 100f, 1500L) // 500ms tap & hold

        assertEquals(2, events.size)
        val down = events[0] as InputEvent.MouseButton
        val up = events[1] as InputEvent.MouseButton

        assertEquals(1, down.button) // Right click
        assertTrue(down.isDown)

        assertEquals(1, up.button)
        assertFalse(up.isDown)
    }
}
