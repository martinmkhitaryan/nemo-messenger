package org.nemo

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.nemo.GroupView
import uniffi.nemo.NemoClient

/**
 * Reload per-member watermarks for every group. A failed lookup keeps the
 * previous value so a busy core does not blank the ticks.
 */
internal suspend fun syncGroupViews(client: NemoClient, into: MutableMap<String, GroupView>, enabled: Boolean) {
    if (!enabled) {
        if (into.isNotEmpty()) into.clear()
        return
    }
    val views = withContext(Dispatchers.IO) {
        val groups = runCatching { client.listGroups() }.getOrNull() ?: return@withContext null
        groups.map { g -> g.groupId to runCatching { client.groupView(g.groupId) }.getOrNull() }
    } ?: return
    val live = views.map { it.first }.toSet()
    into.keys.filter { it !in live }.toList().forEach { into.remove(it) }
    for ((id, view) in views) {
        if (view != null) into[id] = view
    }
}

internal fun groupViewFullyRead(view: GroupView?, seq: ULong): Boolean {
    if (view == null) return false
    return groupFullyRead(view.viewers.map { it.upto }, seq, view.others.toInt())
}

internal fun seenMarks(view: GroupView?): List<SeenMark> {
    if (view == null) return emptyList()
    return view.viewers.map { SeenMark(it.identityId, it.name, it.upto) }
}

/** One other member's viewed watermark. */
internal data class SeenMark(val identityId: String, val name: String, val upto: ULong)

/** Names of members whose watermark covers [seq], in display order. */
internal fun seenNames(marks: List<SeenMark>, seq: ULong, contacts: Map<String, String>): List<String> = marks
    .filter { it.upto >= seq }
    .map { mark ->
        contacts[mark.identityId]?.takeIf { it.isNotBlank() }
            ?: mark.name.takeIf { it.isNotBlank() }
            ?: shortId(mark.identityId)
    }
    .sortedBy { it.lowercase() }

/** Full-emphasis tick: every other current member has this message covered. */
internal fun groupFullyRead(viewerUptos: List<ULong>, seq: ULong, others: Int): Boolean {
    if (others <= 0) return false
    return viewerUptos.count { it >= seq } >= others
}
