package org.nemo

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import java.awt.Toolkit
import java.awt.datatransfer.StringSelection
import java.io.File
import uniffi.nemo.NemoClient

fun main() {
    val root = repoRoot()
    val libDir = File(root, "target/debug")
    if (libDir.resolve("libnemo_ffi.so").isFile || libDir.resolve("nemo_ffi.dll").isFile) {
        System.setProperty("jna.library.path", libDir.absolutePath)
    }
    val data = File(System.getProperty("user.home"), ".local/share/nemo")
    application {
        Window(
            onCloseRequest = ::exitApplication,
            title = "Nemo",
            state = rememberWindowState(width = 1280.dp, height = 800.dp),
        ) {
            NemoTheme {
                Row(Modifier.fillMaxSize()) {
                    SessionPane(
                        label = "Left",
                        vaultDir = File(data, "left"),
                        modifier = Modifier.weight(1f).fillMaxHeight(),
                    )
                    VerticalDivider()
                    SessionPane(
                        label = "Right",
                        vaultDir = File(data, "right"),
                        modifier = Modifier.weight(1f).fillMaxHeight(),
                    )
                }
            }
        }
    }
}

internal fun repoRoot(): File {
    var dir = File(System.getProperty("user.dir")).absoluteFile
    repeat(8) {
        if (File(dir, "crates/nemo-ffi").isDirectory) return dir
        dir = dir.parentFile ?: return File(".")
    }
    return File(".")
}

actual fun pickLocalFile(): String? {
    val dlg = java.awt.FileDialog(null as java.awt.Frame?, "Send file", java.awt.FileDialog.LOAD)
    dlg.isVisible = true
    val dir = dlg.directory
    val name = dlg.file
    if (dir.isNullOrEmpty() || name.isNullOrEmpty()) return null
    return File(dir, name).absolutePath
}

actual fun deviceVaultSecret(vaultDir: File): ByteArray = ByteArray(0)

@Composable
actual fun rememberPickFile(onPicked: (String) -> Unit): () -> Unit {
    return {
        pickLocalFile()?.let(onPicked)
    }
}

@Composable
actual fun rememberScanQr(onText: (String) -> Unit): () -> Unit {
    val latest = rememberUpdatedState(onText)
    return {
        val dlg = java.awt.FileDialog(null as java.awt.Frame?, "Scan QR image", java.awt.FileDialog.LOAD)
        dlg.isVisible = true
        val dir = dlg.directory
        val name = dlg.file
        if (!dir.isNullOrEmpty() && !name.isNullOrEmpty()) {
            val file = java.io.File(dir, name)
            val image = javax.imageio.ImageIO.read(file)
            if (image != null) {
                val w = image.width
                val h = image.height
                val pixels = IntArray(w * h)
                image.getRGB(0, 0, w, h, pixels, 0, w)
                decodeQrArgb(pixels, w, h)?.let(latest.value)
            }
        }
    }
}

actual fun defaultHomeUrl(): String = "https://localhost:8443"

actual fun copyToClipboard(text: String) {
    Toolkit.getDefaultToolkit().systemClipboard.setContents(StringSelection(text), null)
}

actual fun startCallAudio(client: NemoClient) {
    DesktopCallAudio.start(client)
}

actual fun stopCallAudio() {
    DesktopCallAudio.stop()
}

@Composable
actual fun rememberEnsureMic(onReady: () -> Unit): () -> Unit {
    val latest = rememberUpdatedState(onReady)
    return { latest.value.invoke() }
}
