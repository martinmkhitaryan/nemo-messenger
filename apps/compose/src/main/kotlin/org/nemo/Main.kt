package org.nemo

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application

/**
 * Compose Multiplatform shell (ADR-0026 / ADR-0028).
 *
 * Crypto stays in `nemo-core` via `nemo-ffi`. This process must not persist
 * ratchet or MLS keys; display rows only.
 */
fun main() = application {
    Window(onCloseRequest = ::exitApplication, title = "Nemo") {
        MaterialTheme {
            Column(
                modifier = Modifier.fillMaxSize().padding(24.dp),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text("Nemo Messenger")
                Text(
                    "UniFFI client: generate Kotlin from crates/nemo-ffi, then show fingerprint here.",
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
    }
}
