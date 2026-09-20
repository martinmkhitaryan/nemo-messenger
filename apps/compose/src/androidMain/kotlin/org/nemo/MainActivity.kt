package org.nemo

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.Modifier
import java.io.File

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val vault = File(filesDir, "vault")
        setContent {
            MaterialTheme {
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
