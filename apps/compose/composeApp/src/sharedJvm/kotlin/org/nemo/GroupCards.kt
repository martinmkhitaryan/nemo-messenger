package org.nemo

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

/** Contact picker + send button shared by the Join sheet and group Settings.
 * Manual copy-paste stays available next to it; this only adds the direct
 * 1:1 path. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun RelaySendPicker(
    header: String,
    roster: Map<String, String>,
    selectedId: String,
    onSelect: (String) -> Unit,
    contactPlaceholder: String,
    sendLabel: String,
    sendEnabled: Boolean,
    onSend: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    Text(
        header,
        style = MaterialTheme.typography.bodySmall,
        modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
    )
    ExposedDropdownMenuBox(
        expanded = menu,
        onExpandedChange = { menu = it },
        modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
    ) {
        OutlinedTextField(
            value = roster[selectedId] ?: selectedId.ifBlank { "" },
            onValueChange = {},
            readOnly = true,
            label = { Text("1:1 contact") },
            placeholder = { Text(contactPlaceholder) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(menu) },
            modifier = Modifier.fillMaxWidth().menuAnchor(
                ExposedDropdownMenuAnchorType.PrimaryNotEditable,
                enabled = true,
            ),
            singleLine = true,
        )
        ExposedDropdownMenu(
            expanded = menu,
            onDismissRequest = { menu = false },
        ) {
            roster.forEach { (id, name) ->
                DropdownMenuItem(
                    text = { Text(name.ifBlank { shortId(id) }) },
                    onClick = {
                        onSelect(id)
                        menu = false
                    },
                )
            }
        }
    }
    Button(
        enabled = sendEnabled,
        onClick = onSend,
        modifier = Modifier.fillMaxWidth().padding(top = 4.dp),
    ) {
        Text(sendLabel)
    }
}

/** In-chat card for an incoming `nemo-g:1:…` invite. Accept is a one-shot:
 * once taken the card shows the sent state with no buttons. */
@Composable
internal fun GroupInviteCard(
    mine: Boolean,
    timeLabel: String,
    bodyColor: Color,
    metaColor: Color,
    hasStatus: Boolean,
    status: @Composable () -> Unit,
    onAccept: (() -> Unit)?,
    onCopy: (() -> Unit)?,
    accepted: Boolean = false,
) {
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(
            text = "Group invite",
            style = MaterialTheme.typography.bodyLarge,
            fontWeight = FontWeight.SemiBold,
            color = bodyColor,
        )
        Text(
            text = if (mine) {
                "Sent — they tap Accept in this chat."
            } else if (accepted) {
                "Request sent — the inviter taps Admit."
            } else {
                "A group member invites you. Accepting sends a join request back."
            },
            style = MaterialTheme.typography.bodyMedium,
            color = bodyColor,
        )
        if (onAccept != null || onCopy != null) {
            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (onAccept != null) {
                    Button(onClick = onAccept) { Text("Accept") }
                }
                if (onCopy != null) {
                    TextButton(onClick = onCopy) { Text("Copy") }
                }
            }
        }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            Text(text = timeLabel, style = MaterialTheme.typography.labelSmall, color = metaColor)
            if (hasStatus) status()
        }
    }
}

/** In-chat card for an incoming `nemo-j:1:…` join request. Admit is a
 * one-shot: once taken the card shows `Admitted ✓` with no buttons. */
@Composable
internal fun JoinRequestCard(
    mine: Boolean,
    timeLabel: String,
    bodyColor: Color,
    metaColor: Color,
    hasStatus: Boolean,
    status: @Composable () -> Unit,
    onAdmit: (() -> Unit)?,
    onCopy: (() -> Unit)?,
    admitted: Boolean = false,
) {
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(
            text = "Group join request",
            style = MaterialTheme.typography.bodyLarge,
            fontWeight = FontWeight.SemiBold,
            color = bodyColor,
        )
        Text(
            text = if (mine) {
                "Sent — the member taps Admit on their side."
            } else if (admitted) {
                "Admitted ✓"
            } else {
                "Someone asks to join a group you belong to. Verify them, then admit."
            },
            style = MaterialTheme.typography.bodyMedium,
            color = bodyColor,
        )
        if (onAdmit != null || onCopy != null) {
            Row(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (onAdmit != null) {
                    Button(onClick = onAdmit) { Text("Admit") }
                }
                if (onCopy != null) {
                    TextButton(onClick = onCopy) { Text("Copy") }
                }
            }
        }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            Text(text = timeLabel, style = MaterialTheme.typography.labelSmall, color = metaColor)
            if (hasStatus) status()
        }
    }
}
