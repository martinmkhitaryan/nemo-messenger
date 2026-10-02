package org.nemo

import java.io.File

internal const val CARD_MARKS_FILE = "card_marks"

/**
 * Accepted group invites + admitted join requests, keyed by [messageListKey].
 * In-memory sets reset on restart, which resurrects Accept/Admit buttons for
 * already-handled cards — so the marks persist here (one pane = one vault,
 * same pattern as `unread_seen`).
 */
internal fun loadCardMarks(vaultDir: File): Pair<Set<String>, Set<String>> {
    val file = File(vaultDir, CARD_MARKS_FILE)
    if (!file.isFile) return emptySet<String>() to emptySet<String>()
    val accepted = mutableSetOf<String>()
    val admitted = mutableSetOf<String>()
    runCatching {
        file.readLines().forEach { line ->
            when {
                line.startsWith("a ") -> accepted.add(line.removePrefix("a "))
                line.startsWith("j ") -> admitted.add(line.removePrefix("j "))
            }
        }
    }
    return accepted to admitted
}

internal fun saveCardMarks(vaultDir: File, accepted: Set<String>, admitted: Set<String>) {
    runCatching {
        vaultDir.mkdirs()
        val text = buildString {
            for (k in accepted) append("a ").append(k).append('\n')
            for (k in admitted) append("j ").append(k).append('\n')
        }
        File(vaultDir, CARD_MARKS_FILE).writeText(text)
    }
}
