package org.nemo

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import uniffi.nemo.DisplayRow
import uniffi.nemo.NemoClient
import java.io.File
import java.util.IdentityHashMap
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.cancellation.CancellationException

/**
 * Pure merge of fetched rows into a snapshot, mirroring [applyIncoming]
 * without Compose side effects (no audio, no outgoing tracking).
 *
 * Deleted/expired rows hide their target; everything else is deduped by
 * `convId + convSeq + kind`.
 */
internal fun mergeDisplayRows(current: List<DisplayRow>, incoming: List<DisplayRow>): List<DisplayRow> {
    if (incoming.isEmpty()) return current
    val merged = current.toMutableList()
    for (row in incoming) {
        if (row.kind == "deleted" || row.kind == "expired") {
            val idx = merged.indexOfFirst {
                it.convId == row.convId && it.convSeq == row.target && it.kind != "reaction"
            }
            if (idx >= 0) {
                val old = merged[idx]
                merged[idx] = old.copy(hidden = true, kind = row.kind, text = "", fileBytes = byteArrayOf())
            }
        }
        if (merged.none { it.convId == row.convId && it.convSeq == row.convSeq && it.kind == row.kind }) {
            merged.add(row)
        }
    }
    return merged
}

/**
 * Single sync owner per vault.
 *
 * Before this, three loops raced on one destructive `fetchNow()` (two in
 * `SessionPane`, one in `SyncService`): whoever fetched first consumed the
 * server cursor, persisted to the Rust inbox, and left the other collectors
 * with `[]`. The visible symptom was a system notification with a stale
 * chat list until restart (which re-read `inbox()`).
 *
 * Now only the store fetches. The UI collects [messagesFlow] /
 * [contactsFlow] / [groupsFlow]; background delivery collects [freshRows].
 * `fetchMutex` serializes the internal poll loop, wake loop, and any
 * externally triggered [refresh] (service, worker) so consumption happens
 * exactly once and every collector sees the same rows.
 */
internal class NemoVaultStore(val vaultDir: File, @Volatile var client: NemoClient) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val fetchMutex = Mutex()
    private val started = AtomicBoolean(false)
    private var jobs: List<Job> = emptyList()

    val messagesFlow = MutableStateFlow<List<DisplayRow>>(emptyList())
    val contactsFlow = MutableStateFlow<Map<String, String>>(emptyMap())
    val groupsFlow = MutableStateFlow<Map<String, String>>(emptyMap())

    /** Currently open conversation, or null on the list page / background. */
    val visibleChatId = MutableStateFlow<String?>(null)

    /** Raw deltas from the last fetch, for notification layers to filter. */
    val freshRows = MutableSharedFlow<List<DisplayRow>>(extraBufferCapacity = 64)

    fun seed(inbox: List<DisplayRow>, contacts: Map<String, String>, groups: Map<String, String>) {
        messagesFlow.value = mergeDisplayRows(messagesFlow.value, inbox)
        if (contacts.isNotEmpty()) contactsFlow.value = contacts
        if (groups.isNotEmpty()) groupsFlow.value = groups
    }

    /** One fetch cycle: mailbox + expiry + roster, published to all collectors. */
    suspend fun refresh(): List<DisplayRow> {
        val c = client
        return fetchMutex.withLock {
            val rows = withContext(Dispatchers.IO) { runCatching { c.fetchNow() }.getOrDefault(emptyList()) }
            val expired = withContext(Dispatchers.IO) { runCatching { c.expireNow() }.getOrDefault(emptyList()) }
            val newRows = rows + expired
            if (newRows.isNotEmpty()) {
                messagesFlow.update { mergeDisplayRows(it, newRows) }
                runCatching { freshRows.emit(newRows) }
            }
            val contactRows = withContext(Dispatchers.IO) {
                runCatching { c.listContacts() }.getOrDefault(emptyList())
            }
            val groupRows = withContext(Dispatchers.IO) {
                runCatching { c.listGroups() }.getOrDefault(emptyList())
            }
            if (contactRows.isNotEmpty() || groupRows.isNotEmpty()) {
                contactsFlow.value = contactRows.associate { it.identityId to it.nickname }
                groupsFlow.value = groupRows.associate { it.groupId to it.nickname.ifBlank { "Group" } }
            }
            newRows
        }
    }

    /** Full heal from persisted inbox (unlock, Home enter). Cheap deltas stay in [refresh]. */
    suspend fun reconcile() {
        val c = client
        val inbox = withContext(Dispatchers.IO) { runCatching { c.inbox() }.getOrDefault(emptyList()) }
        if (inbox.isNotEmpty()) messagesFlow.update { mergeDisplayRows(it, inbox) }
    }

    fun start() {
        if (!started.compareAndSet(false, true)) return
        val poll = scope.launch {
            while (isActive) {
                delay(2_000)
                try {
                    refresh()
                } catch (e: CancellationException) {
                    throw e
                } catch (_: Throwable) {
                }
            }
        }
        val wake = scope.launch {
            while (isActive) {
                try {
                    val c = client
                    withContext(Dispatchers.IO) { c.waitWakeup() }
                    refresh()
                } catch (e: CancellationException) {
                    throw e
                } catch (_: Throwable) {
                    delay(2_000)
                }
            }
        }
        jobs = listOf(poll, wake)
    }

    fun close() {
        jobs.forEach { it.cancel() }
        scope.cancel()
    }
}

/** Registry keyed by vault path; reverse lookup by client instance for services. */
internal object VaultStores {
    private val lock = Any()
    private val byPath = mutableMapOf<String, NemoVaultStore>()
    private val byClient = IdentityHashMap<NemoClient, NemoVaultStore>()

    fun getOrCreate(vaultDir: File, client: NemoClient): NemoVaultStore {
        synchronized(lock) {
            byClient[client]?.let { return it }
            val key = vaultDir.absolutePath
            val existing = byPath[key]
            if (existing != null) {
                if (existing.client === client) return existing
                runCatching { existing.close() }
                val stale = byClient.entries.iterator()
                while (stale.hasNext()) {
                    if (stale.next().value === existing) stale.remove()
                }
            }
            val store = NemoVaultStore(vaultDir, client)
            byPath[key] = store
            byClient[client] = store
            return store
        }
    }

    fun findByClient(client: NemoClient): NemoVaultStore? {
        synchronized(lock) { return byClient[client] }
    }

    fun findByPath(vaultDir: File): NemoVaultStore? {
        synchronized(lock) { return byPath[vaultDir.absolutePath] }
    }

    fun remove(vaultDir: File) {
        synchronized(lock) {
            val store = byPath.remove(vaultDir.absolutePath) ?: return
            val stale = byClient.entries.iterator()
            while (stale.hasNext()) {
                if (stale.next().value === store) stale.remove()
            }
            runCatching { store.close() }
        }
    }
}

/**
 * Subscribes the Home UI to the single [NemoVaultStore] owner. Lives here
 * (not in Session.kt) to keep `SessionPane` under the method/function budget.
 */
@Composable
internal fun BindVaultStore(
    store: NemoVaultStore?,
    atHome: Boolean,
    messages: MutableList<DisplayRow>,
    contacts: MutableMap<String, String>,
    groups: MutableMap<String, String>,
    outgoing: MutableMap<String, Boolean>,
    outgoingStatus: MutableMap<String, OutgoingStatus>,
    selectedId: String?,
) {
    LaunchedEffect(store, atHome) {
        val s = store ?: return@LaunchedEffect
        if (!atHome) return@LaunchedEffect
        s.start()
        // Seed from the store snapshot, then heal from anything the background
        // persisted while we were away. Deltas stream below via freshRows.
        applyIncoming(messages, s.messagesFlow.value, outgoing, outgoingStatus)
        contacts.clear()
        contacts.putAll(s.contactsFlow.value)
        groups.clear()
        groups.putAll(s.groupsFlow.value)
        try {
            s.reconcile()
        } catch (_: Throwable) {
        }
    }

    LaunchedEffect(store, atHome) {
        val s = store ?: return@LaunchedEffect
        if (!atHome) return@LaunchedEffect
        s.freshRows.collect { rows ->
            applyIncoming(messages, rows, outgoing, outgoingStatus)
        }
    }

    LaunchedEffect(store, atHome) {
        val s = store ?: return@LaunchedEffect
        if (!atHome) return@LaunchedEffect
        s.contactsFlow.collect { map ->
            contacts.clear()
            contacts.putAll(map)
        }
    }

    LaunchedEffect(store, atHome) {
        val s = store ?: return@LaunchedEffect
        if (!atHome) return@LaunchedEffect
        s.groupsFlow.collect { map ->
            groups.clear()
            groups.putAll(map)
        }
    }

    LaunchedEffect(store, selectedId) {
        store?.visibleChatId?.value = selectedId
    }
}
