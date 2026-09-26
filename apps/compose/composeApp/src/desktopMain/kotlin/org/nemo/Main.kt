package org.nemo

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import uniffi.nemo.NemoClient
import java.awt.Toolkit
import java.awt.datatransfer.StringSelection
import java.io.File

fun main() {
    val root = repoRoot()
    // Prefer path from Gradle (-Djna.library.path). Else release, then debug.
    if (System.getProperty("jna.library.path").isNullOrBlank()) {
        val candidates = listOf(
            File(root, "target/release"),
            File(root, "target/debug"),
        )
        val libDir = candidates.firstOrNull { dir ->
            dir.resolve("libnemo_ffi.so").isFile || dir.resolve("nemo_ffi.dll").isFile
        }
        if (libDir != null) {
            System.setProperty("jna.library.path", libDir.absolutePath)
        }
    }
    val data = File(System.getProperty("user.home"), ".local/share/nemo")
    application {
        Window(
            onCloseRequest = ::exitApplication,
            title = "Nemo",
            icon = painterResource("nemo_logo.png"),
            state = rememberWindowState(width = 1280.dp, height = 800.dp),
        ) {
            var themeMode by remember { mutableStateOf(loadThemeMode()) }
            NemoTheme(
                mode = themeMode,
                onModeChange = {
                    themeMode = it
                    saveThemeMode(it)
                },
            ) {
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

actual fun deviceVaultSecret(vaultPath: String): ByteArray = ByteArray(0)

@Composable
actual fun rememberPickFile(onPicked: (String) -> Unit): () -> Unit = {
    pickLocalFile()?.let(onPicked)
}

@Composable
actual fun rememberSaveFile(onResult: (Boolean) -> Unit): (fileName: String, bytes: ByteArray) -> Unit {
    val latest = rememberUpdatedState(onResult)
    return { fileName, bytes ->
        val suggested = fileName.ifBlank { "attachment" }
        val dlg = java.awt.FileDialog(null as java.awt.Frame?, "Save file", java.awt.FileDialog.SAVE)
        dlg.file = suggested
        dlg.isVisible = true
        val dir = dlg.directory
        val name = dlg.file
        if (dir.isNullOrEmpty() || name.isNullOrEmpty()) {
            latest.value.invoke(false)
        } else {
            try {
                File(dir, name).writeBytes(bytes)
                latest.value.invoke(true)
            } catch (_: Throwable) {
                latest.value.invoke(false)
            }
        }
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

@Suppress("UNUSED_PARAMETER")
@Composable
actual fun NemoBackHandler(enabled: Boolean, onBack: () -> Unit) {
    // Desktop has no system back gesture; Escape / window chrome stay as-is.
}

internal actual fun messageSendFlyDurationMs(): Int = 420

internal actual fun messageSendFlyMobileFeel(): Boolean = false

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
