package org.nemo

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.nemo.NemoClient

@Composable
internal fun AddContactSheet(client: NemoClient?, busy: Boolean, onAdd: (String, String) -> Unit) {
    var cardPaste by remember { mutableStateOf("") }
    var cardPreviewFp by remember { mutableStateOf("") }
    var nickname by remember { mutableStateOf("") }
    val scanQr = rememberScanQr { text ->
        cardPaste = text.trim()
    }
    val nameFocus = remember { FocusRequester() }
    LaunchedEffect(cardPaste, client) {
        val c = client
        val raw = cardPaste.trim()
        cardPreviewFp = if (c == null || raw.isEmpty()) {
            ""
        } else {
            withContext(Dispatchers.IO) {
                runCatching { c.previewContact(raw).fingerprint }.getOrDefault("")
            }
        }
    }
    SheetForm(
        title = "New chat",
        action = "Add",
        enabled = !busy && cardPaste.isNotBlank(),
        onAction = { onAdd(cardPaste, nickname) },
    ) {
        OutlinedTextField(
            value = cardPaste,
            onValueChange = { cardPaste = it },
            label = { Text("Contact card") },
            placeholder = { Text("nemo:1:…") },
            supportingText = { Text("Paste the card your contact shared.") },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
            keyboardActions = KeyboardActions(onNext = { nameFocus.requestFocus() }),
        )
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            TextButton(onClick = scanQr) {
                Text("Scan QR")
            }
            if (cardPaste.isNotEmpty()) {
                TextButton(onClick = {
                    cardPaste = ""
                    cardPreviewFp = ""
                }) {
                    Text("Clear")
                }
            }
        }
        if (cardPreviewFp.isNotEmpty()) {
            Surface(
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(12.dp),
                color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.6f),
            ) {
                Column(Modifier.padding(horizontal = 12.dp, vertical = 10.dp)) {
                    Text(
                        "Verify contact",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    Spacer(Modifier.height(2.dp))
                    Text(
                        "Check this matches what your contact sees, then add them.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.height(6.dp))
                    SelectionContainer {
                        Text(
                            cardPreviewFp,
                            fontFamily = FontFamily.Monospace,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                    }
                }
            }
            Spacer(Modifier.height(12.dp))
        }
        OutlinedTextField(
            value = nickname,
            onValueChange = { nickname = it },
            label = { Text("Name") },
            modifier = Modifier.fillMaxWidth().focusRequester(nameFocus),
            singleLine = true,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        )
    }
}
