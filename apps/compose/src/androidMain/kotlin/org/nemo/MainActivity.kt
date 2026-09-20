package org.nemo

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import java.io.File

private var appContext: Context? = null

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        appContext = applicationContext
        enableEdgeToEdge()
        val vault = File(filesDir, "vault")
        setContent {
            NemoTheme {
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

actual fun defaultHomeUrl(): String = "https://10.0.2.2:8443"

actual fun copyToClipboard(text: String) {
    val ctx = appContext ?: return
    val clipboard = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    clipboard.setPrimaryClip(ClipData.newPlainText("nemo", text))
}
