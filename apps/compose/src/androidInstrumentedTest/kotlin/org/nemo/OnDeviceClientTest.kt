package org.nemo

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.nemo.NemoClient
import java.io.File
import java.net.URI
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlin.test.fail

@RunWith(AndroidJUnit4::class)
class OnDeviceClientTest {
    @Test
    fun vaultCreateOpenWrongPassphrase() {
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val dir = File(ctx.cacheDir, "vault-${System.nanoTime()}")
        dir.mkdirs()
        val pass = "correct horse"
        val secret = deviceVaultSecret(dir)
        val client = NemoClient.createAt(dir.absolutePath, pass, secret)
        val id = client.identityIdHex()
        assertEquals(64, id.length)
        val mnemonic = client.takeRevocationMnemonic()
        assertNotNull(mnemonic)
        assertTrue(mnemonic.split(" ").size >= 12)
        assertNull(client.takeRevocationMnemonic())
        client.close()

        val opened = NemoClient.openAt(dir.absolutePath, pass, secret)
        assertEquals(id, opened.identityIdHex())
        opened.close()

        val bad = runCatching { NemoClient.openAt(dir.absolutePath, "incorrect!!", secret) }
        assertTrue(bad.isFailure)
        dir.deleteRecursively()
    }

    @Test
    fun twoVaultsExchangeTextAndFile() {
        val home = requireHome()
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val leftDir = File(ctx.cacheDir, "ex-l-${System.nanoTime()}").also { it.mkdirs() }
        val rightDir = File(ctx.cacheDir, "ex-r-${System.nanoTime()}").also { it.mkdirs() }
        val pass = "correct horse"
        val leftSecret = deviceVaultSecret(leftDir)
        val rightSecret = deviceVaultSecret(rightDir)
        val left = NemoClient.createAt(leftDir.absolutePath, pass, leftSecret)
        val right = NemoClient.createAt(rightDir.absolutePath, pass, rightSecret)
        try {
            left.takeRevocationMnemonic()
            right.takeRevocationMnemonic()
            left.register(home)
            right.register(home)
            val uri = left.mintShareUri()
            assertEquals(left.fingerprint(), right.previewContact(uri).fingerprint)
            val leftId = left.identityIdHex()
            right.addContact(uri, "Left")
            right.sendText(leftId, "hello from right")
            val first = left.fetchNow()
            assertEquals(1, first.size)
            assertEquals("hello from right", first[0].text)
            right.sendFile(leftId, "note.txt", "text/plain", "hello file".toByteArray())
            val file = left.fetchNow()
            assertEquals("note.txt", file[0].fileName)
            assertEquals("hello file", file[0].fileBytes.decodeToString())
            left.close()
            right.close()
            val left2 = NemoClient.openAt(leftDir.absolutePath, pass, leftSecret)
            val right2 = NemoClient.openAt(rightDir.absolutePath, pass, rightSecret)
            right2.sendText(leftId, "still there")
            assertEquals("still there", left2.fetchNow()[0].text)
            left2.close()
            right2.close()
        } finally {
            leftDir.deleteRecursively()
            rightDir.deleteRecursively()
        }
    }

    @Test
    fun groupInviteAdmitAndRemove() {
        val home = requireHome()
        val ctx = InstrumentationRegistry.getInstrumentation().targetContext
        val aliceDir = File(ctx.cacheDir, "g-a-${System.nanoTime()}").also { it.mkdirs() }
        val bobDir = File(ctx.cacheDir, "g-b-${System.nanoTime()}").also { it.mkdirs() }
        val carolDir = File(ctx.cacheDir, "g-c-${System.nanoTime()}").also { it.mkdirs() }
        val pass = "correct horse"
        val alice = NemoClient.createAt(aliceDir.absolutePath, pass, deviceVaultSecret(aliceDir))
        val bob = NemoClient.createAt(bobDir.absolutePath, pass, deviceVaultSecret(bobDir))
        val carol = NemoClient.createAt(carolDir.absolutePath, pass, deviceVaultSecret(carolDir))
        try {
            alice.takeRevocationMnemonic()
            bob.takeRevocationMnemonic()
            carol.takeRevocationMnemonic()
            alice.register(home)
            bob.register(home)
            carol.register(home)
            val gid = alice.createGroup("crew")
            val invite = alice.mintGroupInvite(gid)
            val join = bob.acceptGroupInvite(invite)
            val carolDenied = runCatching { carol.acceptGroupInvite(invite) }.exceptionOrNull()
            assertTrue(carolDenied?.message.orEmpty().lowercase().contains("denied"))
            alice.admitJoin(join)
            bob.fetchNow()
            alice.sendGroupText(gid, "hello crew")
            val first = bob.fetchNow()
            assertEquals("hello crew", first[0].text)
            assertEquals(gid, first[0].convId)
        } finally {
            alice.close()
            bob.close()
            carol.close()
            aliceDir.deleteRecursively()
            bobDir.deleteRecursively()
            carolDir.deleteRecursively()
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
