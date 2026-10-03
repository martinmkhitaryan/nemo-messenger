package org.nemo

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Reply
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import uniffi.nemo.DisplayRow

/** Peer accent palette (red, orange, violet, green, cyan, blue, pink). */
internal val PEER_ACCENT_COLORS = listOf(
    Color(0xFFE17076),
    Color(0xFFF5A623),
    Color(0xFF9B7EDE),
    Color(0xFF6BCB77),
    Color(0xFF4DD0E1),
    Color(0xFF579DFF),
    Color(0xFFE77FB0),
)

internal fun peerAccentColor(key: String): Color {
    if (key.isEmpty()) return PEER_ACCENT_COLORS[5]
    val h = key.hashCode().let { if (it == Int.MIN_VALUE) 0 else kotlin.math.abs(it) }
    return PEER_ACCENT_COLORS[h % PEER_ACCENT_COLORS.size]
}

/**
 * Stable accent key for a quoted message: same user must always map to the
 * same color. Never include [DisplayRow.convSeq] — sender ids are blank for
 * own messages, so seq-qualified fallbacks change color per message.
 */
internal fun quoteAccentKey(quote: DisplayRow): String = when {
    quote.outgoing -> "you"
    quote.senderId.isNotBlank() -> quote.senderId
    quote.convId.isNotBlank() -> quote.convId
    else -> "peer"
}

/** Fixed horizontal chrome of [ReplyQuoteHeader]: bar start + bar + gaps + end padding. */
internal const val REPLY_QUOTE_DECOR_DP: Int = 27

internal fun quoteAuthorName(quote: DisplayRow, mine: Boolean, contacts: Map<String, String> = emptyMap()): String = when {
    quote.outgoing || mine -> "You"
    quote.senderName.isNotBlank() -> quote.senderName
    contacts[quote.senderId]?.isNotBlank() == true -> contacts[quote.senderId]!!
    contacts[quote.convId]?.isNotBlank() == true -> contacts[quote.convId]!!
    quote.senderId.isNotBlank() -> shortId(quote.senderId)
    quote.convId.isNotBlank() -> shortId(quote.convId)
    else -> ""
}

/**
 * Reply header inside a bubble: 3dp rounded accent bar + colored
 * sender name + 1-2 line preview. Hidden entirely when the original was
 * hard-deleted (caller passes null).
 */
@Composable
internal fun ReplyQuoteHeader(
    quote: DisplayRow,
    bodyColor: Color,
    onJump: (() -> Unit)? = null,
    contacts: Map<String, String> = emptyMap(),
) {
    val accent = peerAccentColor(quoteAccentKey(quote))
    val author = quoteAuthorName(quote, mine = false, contacts = contacts)
    Row(
        modifier = Modifier
            // Wrap the content instead of stretching the bubble full width:
            // the bubble hugs the wider of quote and message text.
            .width(IntrinsicSize.Max)
            .clip(RoundedCornerShape(8.dp))
            .then(if (onJump != null) Modifier.clickable(onClick = onJump) else Modifier)
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier
                .padding(start = 8.dp)
                .width(3.dp)
                .height(32.dp)
                .clip(RoundedCornerShape(2.dp))
                .background(accent),
        )
        Spacer(Modifier.width(8.dp))
        Column(Modifier.weight(1f).padding(end = 8.dp)) {
            if (author.isNotBlank()) {
                Text(
                    text = author,
                    style = MaterialTheme.typography.labelSmall,
                    fontWeight = FontWeight.SemiBold,
                    color = accent,
                    maxLines = 1,
                )
            }
            Text(
                text = shortQuoteText(
                    if (quote.fileName.isNotEmpty()) "📎 ${quote.fileName}" else quote.text,
                ),
                style = MaterialTheme.typography.bodySmall,
                color = bodyColor.copy(alpha = 0.85f),
                maxLines = 2,
            )
        }
    }
}

/** One fluid reaction cell: pops on press, ring when it's mine. */
@Composable
private fun FluidReactionCell(emoji: String, selected: Boolean, onToggle: () -> Unit) {
    val interactionSource = remember { MutableInteractionSource() }
    val pressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (pressed) 1.35f else 1f,
        animationSpec = spring(
            dampingRatio = Spring.DampingRatioMediumBouncy,
            stiffness = Spring.StiffnessMediumLow,
        ),
        label = "pop",
    )
    Surface(
        shape = CircleShape,
        color = if (selected) MaterialTheme.colorScheme.primaryContainer else Color.Transparent,
        modifier = Modifier.size(44.dp),
    ) {
        Box(
            contentAlignment = Alignment.Center,
            modifier = Modifier
                .scale(scale)
                .clickable(
                    interactionSource = interactionSource,
                    indication = null,
                    onClick = onToggle,
                ),
        ) {
            Text(text = emoji, fontSize = 26.sp)
        }
    }
}

private data class MenuAction(
    val label: String,
    val icon: @Composable () -> Unit,
    val destructive: Boolean = false,
    val onClick: () -> Unit,
)

/**
 * Fluid/crystal context menu content (no window of its own).
 *
 * Top: floating reaction bar (quick emojis, tap toggles, `+` expands to the
 * full picker). Bottom: crystal action card (translucent surface, rounded
 * 18dp) with Reply / Copy / Save / Delete. The caller anchors it near the
 * bubble (see `MessageBubble`) so edge cases flip it above/below instead of
 * centering on screen.
 */
@Composable
internal fun FluidMessageMenuContent(
    pills: List<ReactionPill>,
    canSave: Boolean,
    canCopy: Boolean,
    onQuickReact: (String) -> Unit,
    onExpandReactions: () -> Unit,
    onReply: () -> Unit,
    onCopy: () -> Unit,
    onSave: () -> Unit,
    onDelete: () -> Unit,
) {
    val mineEmojis = pills.filter { it.mine }.map { it.emoji }.toSet()
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier.widthIn(max = 300.dp),
    ) {
        // --- Fluid reaction bar ---
        Surface(
            shape = RoundedCornerShape(28.dp),
            // Near-opaque surface over the chat: floating feel without
            // washing out against busy wallpaper.
            color = MaterialTheme.colorScheme.surface.copy(alpha = 0.97f),
            shadowElevation = 12.dp,
            tonalElevation = 0.dp,
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
                horizontalArrangement = Arrangement.spacedBy(2.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                for (emoji in QUICK_REACTIONS.take(5)) {
                    FluidReactionCell(
                        emoji = emoji,
                        selected = emoji in mineEmojis,
                        onToggle = { onQuickReact(emoji) },
                    )
                }
                // Expand chevron to the full picker.
                Surface(
                    shape = CircleShape,
                    color = MaterialTheme.colorScheme.surfaceVariant,
                    modifier = Modifier.size(44.dp),
                ) {
                    Box(
                        contentAlignment = Alignment.Center,
                        modifier = Modifier.clickable(onClick = onExpandReactions),
                    ) {
                        Icon(
                            imageVector = Icons.Filled.KeyboardArrowDown,
                            contentDescription = "More reactions",
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(26.dp),
                        )
                    }
                }
            }
        }
        Spacer(Modifier.height(12.dp))
        // --- Crystal action card ---
        Surface(
            shape = RoundedCornerShape(18.dp),
            color = MaterialTheme.colorScheme.surface.copy(alpha = 0.98f),
            shadowElevation = 16.dp,
            tonalElevation = 0.dp,
        ) {
            Column(modifier = Modifier.padding(vertical = 6.dp).widthIn(min = 240.dp)) {
                val actions = buildList {
                    add(
                        MenuAction("Reply", { Icon(Icons.AutoMirrored.Filled.Reply, null) }) {
                            onReply()
                        },
                    )
                    if (canCopy) {
                        add(
                            MenuAction("Copy", { Icon(Icons.Filled.ContentCopy, null) }) {
                                onCopy()
                            },
                        )
                    }
                    if (canSave) {
                        add(
                            MenuAction("Save", { Icon(Icons.Filled.Download, null) }) {
                                onSave()
                            },
                        )
                    }
                    add(
                        MenuAction(
                            "Delete",
                            { Icon(Icons.Filled.Delete, null) },
                            destructive = true,
                        ) {
                            onDelete()
                        },
                    )
                }
                for ((i, a) in actions.withIndex()) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable(onClick = a.onClick)
                            .padding(horizontal = 16.dp, vertical = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        val tint = if (a.destructive) {
                            MaterialTheme.colorScheme.error
                        } else {
                            MaterialTheme.colorScheme.onSurface
                        }
                        a.icon()
                        Spacer(Modifier.width(14.dp))
                        Text(
                            text = a.label,
                            color = tint,
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier.weight(1f),
                        )
                    }
                    if (i < actions.lastIndex) {
                        Box(
                            Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 16.dp)
                                .height(0.5.dp)
                                .background(MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f)),
                        )
                    }
                }
            }
        }
    }
}
