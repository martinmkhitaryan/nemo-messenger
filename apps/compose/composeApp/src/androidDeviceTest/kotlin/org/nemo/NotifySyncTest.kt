package org.nemo

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.nemo.NemoClient
import java.io.File
import java.net.URI
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import kotlin.test.fail

/**
 * Background sync against a live home server (same requirement as
 * [OnDeviceClientTest]). Covers fetch → diff → persist integration;
 * notification *content* rules are unit-tested in [NotifyDiffTest].
 */
@RunWith(AndroidJUnit4::class)
class NotifySyncTest {
    @Test
    fun syncNowPersistsSeenAndSecondRunStaysSilent() {
        val home = requireHome()
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val previousAppContext = appContext
        appContext = ctx
        val leftDir = File(ctx.cacheDir, "nb-l-${System.nanoTime()}").also { it.mkdirs() }
        val rightDir = File(ctx.cacheDir, "nb-r-${System.nanoTime()}").also { it.mkdirs() }
        val pass = "correct horse"
        val left = NemoClient.createAt(leftDir.absolutePath, pass, deviceVaultSecret(leftDir.absolutePath))
        val right = NemoClient.createAt(rightDir.absolutePath, pass, deviceVaultSecret(rightDir.absolutePath))
        try {
            left.takeRevocationMnemonic()
            right.takeRevocationMnemonic()
            left.register(home)
            right.register(home)
            val leftId = left.identityIdHex()
            right.addContact(left.mintShareUri(), "Left")
            right.sendText(leftId, "hello bg")

            // POST_NOTIFICATIONS is denied by default: this run must still
            // fetch and persist without crashing, just without posting.
            syncNow(left)
            val seenFile = File(ctx.filesDir, "notify_seen")
            assertTrue(seenFile.isFile, "syncNow must persist the seen high-water mark")
            val before = seenFile.readText()
            assertTrue(before.isNotEmpty())

            syncNow(left)
            assertEquals(before, seenFile.readText(), "second run must not re-notify or rewrite")
        } finally {
            runCatching { left.close() }
            runCatching { right.close() }
            leftDir.deleteRecursively()
            rightDir.deleteRecursively()
            appContext = previousAppContext
        }
    }

    private fun requireHome(): String {
        val args = InstrumentationRegistry.getArguments()
        val home = args.getString("nemo.home") ?: "http://10.0.2.2:18787"
        try {
            URI("$home/v1/bundle").toURL().openStream().use { it.readBytes() }
        } catch (e: Exception) {
            fail(
                "nemo-server not reachable at $home/v1/bundle. " +
                    "On an emulator bind the host with NEMO_LISTEN=0.0.0.0:18787. ${e.message}",
            )
        }
        return home
    }
}
