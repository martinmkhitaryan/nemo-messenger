package org.nemo

import kotlin.test.Test
import kotlin.test.assertEquals

class FriendlyErrorTest {
    @Test
    fun lockContentionNeverLeaksRawBusy() {
        assertEquals(
            "Still working — please try again in a moment.",
            friendlyErrorMessage(IllegalStateException("busy")),
        )
        assertEquals(
            "Still working — please try again in a moment.",
            friendlyErrorMessage(IllegalStateException("lock")),
        )
    }

    @Test
    fun registrationStatesAreExplained() {
        assertEquals(
            "Connect to a home server first.",
            friendlyErrorMessage(IllegalStateException("not registered with a home server")),
        )
        assertEquals(
            "Already connected to this home server.",
            friendlyErrorMessage(IllegalStateException("identity already registered on this home")),
        )
        assertEquals(true, isAlreadyRegisteredError(IllegalStateException("identity already registered on this home")))
        assertEquals(false, isAlreadyRegisteredError(IllegalStateException("busy")))
    }

    @Test
    fun transportFailuresPointAtTheAddress() {
        val expected = "Couldn't reach the home server. Check the address and try again."
        assertEquals(expected, friendlyErrorMessage(RuntimeException("connection refused")))
        assertEquals(expected, friendlyErrorMessage(RuntimeException("home transport: connection refused")))
        assertEquals(expected, friendlyErrorMessage(RuntimeException("java.net.ConnectException: Failed to connect to /192.168.1.2")))
    }

    @Test
    fun serverErrorsAreSeparatedFromUnreachable() {
        assertEquals(
            "The home server returned an error. Try again later.",
            friendlyErrorMessage(RuntimeException("home HTTP status 500")),
        )
    }

    @Test
    fun unknownErrorsStayVisible() {
        assertEquals("weird engine detail", friendlyErrorMessage(RuntimeException("weird engine detail")))
    }

    @Test
    fun overlongMessagesAreExplained() {
        assertEquals(
            "Message is too long — keep it under 8192 bytes.",
            friendlyErrorMessage(RuntimeException("application text exceeds 8192 UTF-8 bytes")),
        )
    }

    @Test
    fun draftLimitCountsBytesNotChars() {
        assertEquals(8192, draftByteSize("x".repeat(8192)))
        assertEquals(false, isOverTextLimit("x".repeat(8192)))
        assertEquals(true, isOverTextLimit("x".repeat(8193)))
        assertEquals(true, isOverTextLimit("👍".repeat(2049)))
        assertEquals(false, isOverTextLimit("  "))
    }
}
