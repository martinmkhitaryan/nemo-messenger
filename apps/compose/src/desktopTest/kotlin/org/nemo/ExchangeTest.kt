package org.nemo

import uniffi.nemo.NemoClient
import java.io.File
import java.net.URI
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ExchangeTest {
    @Test
    fun twoVaultDirsExchangeTextViaLocalServer() {
        val root = repoRoot()
        val lib = File(root, "target/debug/libnemo_ffi.so")
        assertTrue(lib.isFile, "build nemo-ffi first (libnemo_ffi.so)")
        System.setProperty("jna.library.path", lib.parentFile.absolutePath)

        val server = File(root, "target/debug/nemo-server")
        assertTrue(server.isFile, "build nemo-server first")
        val listen = "127.0.0.1:18787"
        val proc = ProcessBuilder(server.absolutePath)
            .directory(root)
            .redirectErrorStream(true)
            .apply {
                environment()["NEMO_LISTEN"] = listen
                environment()["NEMO_S2S_LISTEN"] = "127.0.0.1:19443"
                environment().remove("DATABASE_URL")
            }
            .start()
        try {
            waitForBundle("http://$listen/v1/bundle")
            val leftDir = Files.createTempDirectory("nemo-left-").toFile()
            val rightDir = Files.createTempDirectory("nemo-right-").toFile()
            val pass = "correct horse"
            val home = "http://$listen"

            val left = NemoClient.createAt(leftDir.absolutePath, pass)
            val right = NemoClient.createAt(rightDir.absolutePath, pass)
            left.takeRevocationMnemonic()
            right.takeRevocationMnemonic()
            left.register(home)
            right.register(home)
            val uri = left.mintShareUri()
            assertTrue(uri.startsWith("nemo:1:"))
            val preview = right.previewContact(uri)
            assertEquals(left.fingerprint(), preview.fingerprint)
            val leftId = left.identityIdHex()
            right.addContact(uri, "Left")
            right.sendText(leftId, "hello from right")
            val first = left.fetchNow()
            assertEquals(1, first.size)
            assertEquals("hello from right", first[0].text)
            right.sendFile(leftId, "note.txt", "text/plain", "hello file".toByteArray())
            val file = left.fetchNow()
            assertEquals(1, file.size)
            assertEquals("note.txt", file[0].fileName)
            assertEquals("hello file", file[0].fileBytes.decodeToString())

            left.close()
            right.close()

            val left2 = NemoClient.openAt(leftDir.absolutePath, pass)
            val right2 = NemoClient.openAt(rightDir.absolutePath, pass)
            right2.sendText(leftId, "still there")
            val second = left2.fetchNow()
            assertEquals(1, second.size)
            assertEquals("still there", second[0].text)
            left2.close()
            right2.close()
            leftDir.deleteRecursively()
            rightDir.deleteRecursively()
        } finally {
            proc.destroy()
            proc.waitFor()
        }
    }

    @Test
    fun threeClientsGroupJoinRestartAndRemove() {
        val root = repoRoot()
        val lib = File(root, "target/debug/libnemo_ffi.so")
        assertTrue(lib.isFile, "build nemo-ffi first (libnemo_ffi.so)")
        System.setProperty("jna.library.path", lib.parentFile.absolutePath)

        val server = File(root, "target/debug/nemo-server")
        assertTrue(server.isFile, "build nemo-server first")
        val listen = "127.0.0.1:18788"
        val proc = ProcessBuilder(server.absolutePath)
            .directory(root)
            .redirectErrorStream(true)
            .apply {
                environment()["NEMO_LISTEN"] = listen
                environment()["NEMO_S2S_LISTEN"] = "127.0.0.1:19444"
                environment().remove("DATABASE_URL")
            }
            .start()
        try {
            waitForBundle("http://$listen/v1/bundle")
            val aliceDir = Files.createTempDirectory("nemo-ga-").toFile()
            val bobDir = Files.createTempDirectory("nemo-gb-").toFile()
            val carolDir = Files.createTempDirectory("nemo-gc-").toFile()
            val pass = "correct horse"
            val home = "http://$listen"

            val alice = NemoClient.createAt(aliceDir.absolutePath, pass)
            val bob = NemoClient.createAt(bobDir.absolutePath, pass)
            val carol = NemoClient.createAt(carolDir.absolutePath, pass)
            alice.takeRevocationMnemonic()
            bob.takeRevocationMnemonic()
            carol.takeRevocationMnemonic()
            alice.register(home)
            bob.register(home)
            carol.register(home)

            val gid = alice.createGroup("crew")
            val invite = alice.mintGroupInvite(gid)
            assertTrue(invite.startsWith("nemo-g:1:"))
            val join = bob.acceptGroupInvite(invite)
            assertTrue(join.startsWith("nemo-j:1:"))
            val carolDenied = runCatching { carol.acceptGroupInvite(invite) }.exceptionOrNull()
            assertTrue(carolDenied?.message.orEmpty().lowercase().contains("denied"))

            val bobCred = alice.admitJoin(join)
            bob.fetchNow()
            alice.sendGroupText(gid, "hello crew")
            val first = bob.fetchNow()
            assertEquals(1, first.size)
            assertEquals("hello crew", first[0].text)
            assertEquals(gid, first[0].convId)

            val sentFile = alice.sendGroupFile(gid, "crew.bin", "application/octet-stream", "group-bytes".toByteArray())
            val groupFile = bob.fetchNow().first { it.fileName == "crew.bin" }
            assertEquals("group-bytes", groupFile.fileBytes.decodeToString())

            alice.close()
            val alice2 = NemoClient.openAt(aliceDir.absolutePath, pass)
            alice2.sendGroupText(gid, "after reopen")
            val second = bob.fetchNow()
            assertEquals(1, second.size)
            assertEquals("after reopen", second[0].text)

            alice2.removeGroupMember(gid, bobCred)
            val bobDenied = runCatching { bob.sendGroupText(gid, "still here") }.exceptionOrNull()
            assertTrue(bobDenied?.message.orEmpty().lowercase().contains("denied"))
            val bobDeniedFile = runCatching { bob.fetchGroupFile(gid, sentFile.fetchToken) }.exceptionOrNull()
            assertTrue(bobDeniedFile?.message.orEmpty().lowercase().contains("denied"))

            alice2.close()
            bob.close()
            carol.close()
            aliceDir.deleteRecursively()
            bobDir.deleteRecursively()
            carolDir.deleteRecursively()
        } finally {
            proc.destroy()
            proc.waitFor()
        }
    }

    private fun repoRoot(): File {
        var dir = File(System.getProperty("user.dir")).absoluteFile
        repeat(8) {
            if (File(dir, "crates/nemo-ffi").isDirectory) return dir
            dir = dir.parentFile ?: return File(".")
        }
        return File(".")
    }

    private fun waitForBundle(url: String) {
        val deadline = System.currentTimeMillis() + 15_000
        var last: Exception? = null
        while (System.currentTimeMillis() < deadline) {
            try {
                URI(url).toURL().openStream().use { it.readBytes() }
                return
            } catch (e: Exception) {
                last = e
                Thread.sleep(100)
            }
        }
        throw IllegalStateException("server did not start", last)
    }
}
