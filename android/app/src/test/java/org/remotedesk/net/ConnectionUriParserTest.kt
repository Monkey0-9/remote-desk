package org.remotedesk.net

import org.junit.Assert.*
import org.junit.Test

class ConnectionUriParserTest {

    @Test
    fun testParseAppSchemeWithMetadata() {
        val target = ConnectionUriParser.parse("remotedesk://192.168.1.11:4443?id=rd-8623-8185&name=WorkPC")
        assertEquals("192.168.1.11", target.host)
        assertEquals(4443, target.port)
        assertFalse(target.isSignaling)
        assertEquals("rd-8623-8185", target.targetHostId)
        assertEquals("WorkPC", target.friendlyName)
    }

    @Test
    fun testParseSimpleAppScheme() {
        val target = ConnectionUriParser.parse("remotedesk://10.0.0.5:8080")
        assertEquals("10.0.0.5", target.host)
        assertEquals(8080, target.port)
        assertFalse(target.isSignaling)
        assertEquals("10.0.0.5:8080", target.rawCleanAddress)
    }

    @Test
    fun testParseHttpWebPortalUrl() {
        val target = ConnectionUriParser.parse("http://192.168.1.50:4444/")
        assertEquals("192.168.1.50", target.host)
        assertEquals(4444, target.port)
        assertFalse(target.isSignaling)
    }

    @Test
    fun testParseRawIpAndPort() {
        val target = ConnectionUriParser.parse("192.168.1.200:4443")
        assertEquals("192.168.1.200", target.host)
        assertEquals(4443, target.port)
        assertFalse(target.isSignaling)
    }

    @Test
    fun testParseRawIpWithoutPortDefaultsTo4443() {
        val target = ConnectionUriParser.parse("192.168.1.200")
        assertEquals("192.168.1.200", target.host)
        assertEquals(4443, target.port)
        assertFalse(target.isSignaling)
    }

    @Test
    fun testParseDeviceIdRoutesToSignaling() {
        val target = ConnectionUriParser.parse("rd-8623-8185")
        assertTrue(target.isSignaling)
        assertEquals("rd-8623-8185", target.targetHostId)
    }

    @Test
    fun testParseJsonQrCodePayload() {
        val json = """{"scheme":"remotedesk","id":"rd-9999-1111","ip":"192.168.2.10","port":9000,"name":"Studio Workstation"}"""
        val target = ConnectionUriParser.parse(json)
        assertEquals("192.168.2.10", target.host)
        assertEquals(9000, target.port)
        assertFalse(target.isSignaling)
        assertEquals("rd-9999-1111", target.targetHostId)
        assertEquals("Studio Workstation", target.friendlyName)
    }
}
