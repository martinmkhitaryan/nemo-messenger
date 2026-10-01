package org.nemo

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog

internal val MenagerieCaptions = listOf(
    "These emojis verify absolutely nothing. Enjoy them anyway.",
    "Same four every time. That is the whole trick.",
    "No eavesdroppers were consulted.",
    "If these look different, you are in the wrong app.",
)

// System emojis only: parrot stands in for cockatoo.
// Unicode has no cockatoo.
internal const val MENAGERIE_EMOJIS = "🦜🦝🐬🐼"

@Composable
internal fun MenagerieRow(modifier: Modifier = Modifier) {
    Row(
        modifier = modifier,
        horizontalArrangement = Arrangement.spacedBy(12.dp, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        for (emoji in listOf("🦜", "🦝", "🐬", "🐼")) {
            Box(Modifier.size(56.dp), contentAlignment = Alignment.Center) {
                Text(emoji, fontSize = 40.sp)
            }
        }
    }
}

@Composable
internal fun MenagerieEasterEggDialog(onDismiss: () -> Unit) {
    val caption = remember { MenagerieCaptions.random() }
    Dialog(onDismissRequest = onDismiss) {
        Surface(shape = RoundedCornerShape(24.dp), tonalElevation = 0.dp, shadowElevation = 8.dp) {
            Column(
                Modifier.widthIn(min = 280.dp, max = 340.dp).padding(horizontal = 28.dp, vertical = 24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                MenagerieRow()
                Spacer(Modifier.height(16.dp))
                Text(
                    caption,
                    modifier = Modifier.fillMaxWidth(),
                    textAlign = TextAlign.Center,
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                    lineHeight = 22.sp,
                )
                Spacer(Modifier.height(12.dp))
                Text(
                    "Tap anywhere else to close",
                    textAlign = TextAlign.Center,
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.65f),
                )
            }
        }
    }
}
