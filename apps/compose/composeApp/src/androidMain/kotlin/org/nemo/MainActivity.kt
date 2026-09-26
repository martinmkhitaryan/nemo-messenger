package org.nemo

import android.Manifest
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import uniffi.nemo.NemoClient
import java.io.File

internal var appContext: Context? = null

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        appContext = applicationContext
        enableEdgeToEdge()
        val vault = File(filesDir, "vault")
        setContent {
            var themeMode by remember { mutableStateOf(loadThemeMode()) }
            NemoTheme(
                mode = themeMode,
                onModeChange = {
                    themeMode = it
                    saveThemeMode(it)
                },
            ) {
                SessionPane(
                    label = "Nemo",
                    vaultDir = vault,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }
    }
}

actual fun pickLocalFile(): String? = null

@Composable
actual fun rememberPickFile(onPicked: (String) -> Unit): () -> Unit {
    val ctx = LocalContext.current
    val latest = rememberUpdatedState(onPicked)
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        val name = uri.lastPathSegment?.substringAfterLast('/') ?: "attachment"
        val out = File(ctx.cacheDir, "nemo-$name")
        ctx.contentResolver.openInputStream(uri)?.use { input ->
            out.outputStream().use { input.copyTo(it) }
        } ?: return@rememberLauncherForActivityResult
        latest.value.invoke(out.absolutePath)
    }
    return remember(launcher) {
        { launcher.launch("*/*") }
    }
}

@Composable
actual fun rememberSaveFile(onResult: (Boolean) -> Unit): (fileName: String, bytes: ByteArray) -> Unit {
    val ctx = LocalContext.current
    val latest = rememberUpdatedState(onResult)
    var pending by remember { mutableStateOf<ByteArray?>(null) }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("*/*")) { uri ->
        val bytes = pending
        pending = null
        if (uri == null || bytes == null) {
            latest.value.invoke(false)
            return@rememberLauncherForActivityResult
        }
        try {
            ctx.contentResolver.openOutputStream(uri)?.use { it.write(bytes) }
                ?: run {
                    latest.value.invoke(false)
                    return@rememberLauncherForActivityResult
                }
            latest.value.invoke(true)
        } catch (_: Throwable) {
            latest.value.invoke(false)
        }
    }
    return remember(launcher) {
        { fileName, bytes ->
            pending = bytes
            launcher.launch(fileName.ifBlank { "attachment" })
        }
    }
}

@Composable
actual fun rememberScanQr(onText: (String) -> Unit): () -> Unit {
    val ctx = LocalContext.current
    val latest = rememberUpdatedState(onText)
    val photo = rememberLauncherForActivityResult(ActivityResultContracts.TakePicturePreview()) { bitmap ->
        if (bitmap == null) return@rememberLauncherForActivityResult
        val w = bitmap.width
        val h = bitmap.height
        val pixels = IntArray(w * h)
        bitmap.getPixels(pixels, 0, w, 0, 0, w, h)
        decodeQrArgb(pixels, w, h)?.let(latest.value::invoke)
    }
    val camera = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) photo.launch(null)
    }
    return remember(photo, camera) {
        {
            when (ContextCompat.checkSelfPermission(ctx, Manifest.permission.CAMERA)) {
                PackageManager.PERMISSION_GRANTED -> photo.launch(null)
                else -> camera.launch(Manifest.permission.CAMERA)
            }
        }
    }
}

actual fun defaultHomeUrl(): String = "https://10.0.2.2:8443"

actual fun copyToClipboard(text: String) {
    val ctx = appContext ?: return
    val clipboard = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    clipboard.setPrimaryClip(ClipData.newPlainText("nemo", text))
}

@Composable
actual fun NemoBackHandler(enabled: Boolean, onBack: () -> Unit) {
    androidx.activity.compose.BackHandler(enabled = enabled, onBack = onBack)
}

internal actual fun messageSendFlyDurationMs(): Int = 480

internal actual fun messageSendFlyMobileFeel(): Boolean = true

actual fun startCallAudio(client: NemoClient) {
    val ctx = appContext ?: return
    CallAudio.start(ctx, client)
}

actual fun stopCallAudio() {
    CallAudio.stop()
}

@Composable
actual fun rememberEnsureMic(onReady: () -> Unit): () -> Unit {
    val ctx = LocalContext.current
    val latest = rememberUpdatedState(onReady)
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) latest.value.invoke()
    }
    return remember(launcher) {
        {
            when (ContextCompat.checkSelfPermission(ctx, Manifest.permission.RECORD_AUDIO)) {
                PackageManager.PERMISSION_GRANTED -> latest.value.invoke()
                else -> launcher.launch(Manifest.permission.RECORD_AUDIO)
            }
        }
    }
}
