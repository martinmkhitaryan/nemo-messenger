package org.nemo

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import org.jetbrains.skia.EncodedImageFormat
import uniffi.nemo.DisplayRow
import java.io.File
import kotlin.test.Test

/**
 * Renders group read ticks when NEMO_SEEN_SHOTS is set.
 * Ordinary test runs skip this so CI does not need a display.
 */
class SeenByShots {
    @Test
    fun renderGroupSeenShots() {
        val dir = System.getenv("NEMO_SEEN_SHOTS") ?: return
        File(dir).mkdirs()
        writeShot(File(dir, "ticks-light.png"), dark = false, overlay = Overlay.None)
        writeShot(File(dir, "ticks-dark.png"), dark = true, overlay = Overlay.None)
        writeShot(File(dir, "ticks-menu.png"), dark = false, overlay = Overlay.Menu)
        writeShot(File(dir, "ticks-who.png"), dark = false, overlay = Overlay.Who)
    }

    private fun writeShot(file: File, dark: Boolean, overlay: Overlay) {
        val scene = ImageComposeScene(
            width = 840,
            height = 1200,
            density = Density(2f),
        ) {
            NemoTheme(mode = if (dark) NemoThemeMode.Dark else NemoThemeMode.Light, onModeChange = {}) {
                SeenThread(overlay)
            }
        }
        try {
            val image = scene.render()
            val data = image.encodeToData(EncodedImageFormat.PNG) ?: error("png encode failed")
            file.writeBytes(data.bytes)
        } finally {
            scene.close()
        }
    }
}

private enum class Overlay { None, Menu, Who }

@Composable
private fun SeenThread(overlay: Overlay) {
    Box(Modifier.fillMaxSize()) {
        ChatWallpaper(Modifier.fillMaxSize())
        Column(Modifier.fillMaxSize()) {
            Column(
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 14.dp),
            ) {
                Text(
                    text = "Weekend",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Text(
                    text = "4 members",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Column(
                Modifier.padding(horizontal = 8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                bubble(mine = false, text = "See you at 7?", sender = "Bob", status = null)
                bubble(mine = true, text = "On my way", sender = "", status = OutgoingStatus.Sent)
                bubble(mine = true, text = "Grabbing coffee first", sender = "", status = OutgoingStatus.Sent)
                bubble(mine = true, text = "I'm outside", sender = "", status = OutgoingStatus.Read)
                bubble(mine = true, text = "Save me a seat", sender = "", status = OutgoingStatus.Sent)
            }
        }
        if (overlay == Overlay.Menu) {
            Box(Modifier.fillMaxSize().padding(top = 96.dp, end = 12.dp), contentAlignment = Alignment.TopEnd) {
                FluidMessageMenuContent(
                    pills = emptyList(),
                    canSave = false,
                    canCopy = true,
                    onQuickReact = {},
                    onExpandReactions = {},
                    onReply = {},
                    onCopy = {},
                    onSave = {},
                    onDelete = {},
                    onSeenBy = {},
                )
            }
        }
        if (overlay == Overlay.Who) {
            Box(
                Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.32f)),
                contentAlignment = Alignment.Center,
            ) {
                Surface(
                    shape = RoundedCornerShape(28.dp),
                    color = MaterialTheme.colorScheme.surface,
                    shadowElevation = 6.dp,
                ) {
                    Column(Modifier.padding(24.dp).width(260.dp)) {
                        Text("Seen by", style = MaterialTheme.typography.headlineSmall)
                        Column(Modifier.padding(top = 12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            Text("Bob")
                            Text("Cara")
                        }
                        Box(Modifier.fillMaxWidth().padding(top = 8.dp), contentAlignment = Alignment.CenterEnd) {
                            TextButton(onClick = {}) { Text("Close") }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun bubble(mine: Boolean, text: String, sender: String, status: OutgoingStatus?) {
    MessageBubble(
        row = DisplayRow(
            convId = "ab".repeat(32),
            convSeq = 1UL,
            text = text,
            sentAt = 1_760_724_000UL,
            fileName = "",
            fileMime = "",
            fileBytes = byteArrayOf(),
            fetchToken = "",
            kind = "",
            emoji = "",
            target = 0UL,
            hidden = false,
            displayedAt = 1_760_724_000UL,
            outgoing = mine,
            senderId = if (mine) "" else "bb".repeat(32),
            senderName = sender,
            replyTo = 0UL,
        ),
        mine = mine,
        status = status,
        clusteredAbove = false,
        clusteredBelow = false,
        animateEnter = false,
        onReact = {},
        onDelete = {},
        onSave = {},
        showSenderNames = true,
    )
}
