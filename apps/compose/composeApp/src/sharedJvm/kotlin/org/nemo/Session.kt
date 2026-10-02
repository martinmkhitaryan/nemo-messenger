package org.nemo

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.Crossfade
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.EaseOutCubic
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Chat
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.AccessTime
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.ArrowDownward
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Call
import androidx.compose.material.icons.filled.CallEnd
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.CloudOff
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Done
import androidx.compose.material.icons.filled.DoneAll
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.Group
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.PersonAdd
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CenterAlignedTopAppBar
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MenuDefaults
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.VerticalDivider
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.mutableStateSetOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.findRootCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInWindow
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.password
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.nemo.DisplayRow
import uniffi.nemo.NemoClient
import java.io.File
import java.util.Locale
import java.util.concurrent.atomic.AtomicLong

internal const val CANNOT_RECOVER =
    "This identity cannot be recovered or exported. If you lose the passphrase, the keys and history are gone."

/** Quiet create-screen footnote — full policy stays in [CANNOT_RECOVER]. */
internal const val CREATE_FOOTNOTE = "No recovery if you lose this passphrase."

internal const val UNLOCK_FOOTNOTE = "There is no recovery if the passphrase is wrong."

internal fun canSaveAttachment(row: DisplayRow): Boolean = row.fileName.isNotEmpty() && row.fileBytes.isNotEmpty() && !row.hidden

/**
 * Human-readable one-liner for backend/transport failures. Raw engine
 * strings ("busy", connection stack traces) must never reach the UI.
 */
internal fun friendlyErrorMessage(e: Throwable): String {
    val raw = (e.message ?: e.toString()).trim()
    if (raw.equals("busy", ignoreCase = true) || raw.equals("lock", ignoreCase = true)) {
        return "Still working — please try again in a moment."
    }
    val lower = raw.lowercase(Locale.US)
    val networkHints = listOf(
        "connection refused", "connection reset", "connection timed out",
        "timed out", "timeout", "failed to connect", "unable to connect",
        "couldn't connect", "cannot connect", "network is unreachable",
        "no route to host", "unknown host", "name resolution",
        "nodename nor servname", "handshake", "certificate", " tls", "tls ",
        "ssl", "broken pipe", "stream reset", "econn", "enotfound",
        "etimedout", "econnrefused", "econnreset", "ehostunreach",
        "connectexception", "sockettimeout", "unknownhostexception",
    )
    return when {
        "not registered" in lower -> "Connect to a home server first."
        "already registered" in lower -> "Already connected to this home server."
        "exceeds 8192" in lower || "too long" in lower -> "Message is too long — keep it under 8192 bytes."
        "home http status" in lower -> "The home server returned an error. Try again later."
        networkHints.any { it in lower } -> "Couldn't reach the home server. Check the address and try again."
        else -> raw.ifBlank { "Something went wrong. Try again." }
    }
}

internal fun isAlreadyRegisteredError(e: Throwable): Boolean {
    val lower = (e.message ?: e.toString()).lowercase(Locale.US)
    return "already registered" in lower
}

private enum class Phase { Locked, Create, Mnemonic, Home }

private enum class Sheet { None, AddContact, NewGroup, JoinGroup }

/**
 * Screen-transition motion for top-level navigation.
 *
 * All specs share one language: 300 ms [FastOutSlowInEasing] slide + 220 ms fade.
 * Phase changes slide ~1/6 width (subtle), push navigation (list to thread, settings)
 * slides ~1/4 width to read as forward/back. Split-pane thread swaps use [Crossfade]
 * only so the list pane never moves.
 */
private fun phaseOrder(p: Phase): Int = when (p) {
    Phase.Locked -> 0
    Phase.Create -> 1
    Phase.Mnemonic -> 2
    Phase.Home -> 3
}

private const val SCREEN_SLIDE_MS = 300
private const val SCREEN_FADE_MS = 220
private const val THREAD_FADE_MS = 180

/** Seen rows kept above the unread marker on open. */
private const val UNREAD_CONTEXT_ABOVE = 2

private data class ChatTarget(val id: String, val title: String, val isGroup: Boolean)

/** Ticks for outgoing bubbles: single ✓ on network accept, faded ✓✓ on
 * peer delivery, full-emphasis ✓✓ once the peer views it (read receipts). */
internal enum class OutgoingStatus {
    Pending,
    Sent,
    Delivered,
    Read,
    Failed,
}

private val localMsgSeq = AtomicLong(0)

/** Composer → bubble flight: mobile only. See [MessageSendAnimation]. */

/** Map [child] window bounds into [parent]'s local coordinates (parent need not be an ancestor). */
internal fun boundsInParent(parent: LayoutCoordinates, child: LayoutCoordinates): Rect {
    val b = child.boundsInWindow()
    val origin = parent.positionInWindow()
    return Rect(
        left = b.left - origin.x,
        top = b.top - origin.y,
        right = b.right - origin.x,
        bottom = b.bottom - origin.y,
    )
}

/**
 * Keep the newest message pinned to the composer (list start when [reverseLayout] is true).
 * Short threads then sit above the input with empty space above.
 */
private suspend fun LazyListState.animateChatToBottom(animated: Boolean) {
    if (layoutInfo.totalItemsCount <= 0) return
    if (animated) {
        animateScrollToItem(0)
    } else {
        scrollToItem(0)
    }
}

private fun newLocalId(): String = "local:${System.nanoTime()}-${localMsgSeq.incrementAndGet()}"

private fun nowUnixSecs(): ULong = (System.currentTimeMillis() / 1000L).toULong()

private fun optimisticTextRow(chatId: String, text: String, localId: String, replyTo: ULong = 0UL): DisplayRow = DisplayRow(
    convId = chatId,
    convSeq = 0UL,
    text = text,
    sentAt = nowUnixSecs(),
    fileName = "",
    fileMime = "",
    fileBytes = byteArrayOf(),
    fetchToken = localId,
    kind = "",
    emoji = "",
    target = 0UL,
    hidden = false,
    displayedAt = nowUnixSecs(),
    outgoing = true,

    senderId = "",
    senderName = "",
    replyTo = replyTo,
)

private fun optimisticFileRow(chatId: String, fileName: String, bytes: ByteArray, localId: String, replyTo: ULong = 0UL): DisplayRow =
    DisplayRow(
        convId = chatId,
        convSeq = 0UL,
        text = "",
        sentAt = nowUnixSecs(),
        fileName = fileName,
        fileMime = "application/octet-stream",
        fileBytes = bytes,
        fetchToken = localId,
        kind = "",
        emoji = "",
        target = 0UL,
        hidden = false,
        displayedAt = nowUnixSecs(),
        outgoing = true,

        senderId = "",
        senderName = "",
        replyTo = replyTo,
    )

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SessionPane(label: String, vaultDir: File, modifier: Modifier = Modifier) {
    val scope = rememberCoroutineScope()
    val snackbar = remember { SnackbarHostState() }
    var phase by remember {
        mutableStateOf(if (File(vaultDir, "kdf.cbor").isFile) Phase.Locked else Phase.Create)
    }
    var client by remember { mutableStateOf<NemoClient?>(null) }
    var passphrase by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf("") }
    var confirmWipe by remember { mutableStateOf(false) }
    var mnemonic by remember { mutableStateOf<String?>(null) }
    var fingerprint by remember { mutableStateOf("") }
    var identityHex by remember { mutableStateOf("") }
    var homeUrl by remember { mutableStateOf(defaultHomeUrl()) }
    var registered by remember { mutableStateOf(false) }
    var shareUri by remember { mutableStateOf("") }
    var privacyMode by remember { mutableStateOf("normal") }
    var notifyMode by remember { mutableStateOf(loadNotifyMode()) }
    var readReceipts by remember { mutableStateOf(loadReadReceiptsEnabled()) }
    var pendingNotify by remember { mutableStateOf<NemoNotifyMode?>(null) }
    var notificationsAllowed by remember { mutableStateOf(areNotificationsAllowed()) }
    val ensureNotifications = rememberEnsureNotifications {
        pendingNotify?.let { mode ->
            saveNotifyMode(mode)
            notifyMode = mode
            applyNotifyMode(mode)
        }
        pendingNotify = null
    }
    var groupName by remember { mutableStateOf("") }
    var invitePaste by remember { mutableStateOf("") }
    var inviteUri by remember { mutableStateOf("") }
    var inviteSendTo by remember { mutableStateOf("") }
    var joinUri by remember { mutableStateOf("") }
    var joinSendTo by remember { mutableStateOf("") }
    var joinPaste by remember { mutableStateOf("") }
    var memberCred by remember { mutableStateOf("") }
    var draft by remember { mutableStateOf("") }
    var disappearSecs by remember { mutableStateOf("0") }
    var contactNickname by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var sheet by remember { mutableStateOf(Sheet.None) }
    var showSettings by remember { mutableStateOf(false) }
    var selected by remember { mutableStateOf<ChatTarget?>(null) }
    var addMenu by remember { mutableStateOf(false) }
    var chatMenu by remember { mutableStateOf(false) }
    val messages = remember { mutableStateListOf<DisplayRow>() }
    val outgoing = remember { mutableStateMapOf<String, Boolean>() }
    val outgoingStatus = remember { mutableStateMapOf<String, OutgoingStatus>() }
    // Card actions already taken, keyed by messageListKey: accepted invites
    // (join request sent) and completed admits. Hides the action buttons so
    // the same invite/join can't be submitted twice.
    val acceptedInvites = remember { mutableStateSetOf<String>() }
    val admittedJoins = remember { mutableStateSetOf<String>() }
    // Card marks persist across restarts (in-memory sets alone resurrect
    // Accept/Admit buttons for already-handled cards).
    LaunchedEffect(vaultDir) {
        val (accepted, admitted) = withContext(Dispatchers.IO) { loadCardMarks(vaultDir) }
        if (accepted.isNotEmpty()) acceptedInvites.addAll(accepted)
        if (admitted.isNotEmpty()) admittedJoins.addAll(admitted)
    }
    LaunchedEffect(acceptedInvites.size, admittedJoins.size) {
        val accepted = acceptedInvites.toSet()
        val admitted = admittedJoins.toSet()
        withContext(Dispatchers.IO) { saveCardMarks(vaultDir, accepted, admitted) }
    }
    val contacts = remember { mutableStateMapOf<String, String>() }
    val groups = remember { mutableStateMapOf<String, String>() }
    val lastRead = remember { mutableStateMapOf<String, ULong>() }

    fun runIo(block: suspend () -> Unit) {
        if (busy) return
        busy = true
        scope.launch {
            try {
                block()
            } catch (e: Throwable) {
                snackbar.showSnackbar(friendlyErrorMessage(e))
            } finally {
                busy = false
            }
        }
    }

    fun markOutgoing(row: DisplayRow, status: OutgoingStatus = OutgoingStatus.Sent) {
        val key = outgoingMapKey(row)
        outgoing[key] = true
        outgoingStatus[key] = status
    }

    // Shared by both thread hosts: manual fallback when an invite arrives
    // outside a 1:1 and the join request can't auto-send.
    fun acceptFallback(joinRequest: String) {
        runIo {
            copyToClipboard(joinRequest)
            joinUri = joinRequest
            joinSendTo = ""
            sheet = Sheet.JoinGroup
            snackbar.showSnackbar("Invite accepted — pick a member")
        }
    }

    fun reloadRoster(c: NemoClient) {
        contacts.clear()
        c.listContacts().forEach { contacts[it.identityId] = it.nickname }
        groups.clear()
        c.listGroups().forEach { groups[it.groupId] = it.nickname.ifBlank { "Group" } }
    }

    fun chats(): List<ChatTarget> {
        val fromMessages = messages.map { it.convId }.distinct()
        val ids = (contacts.keys + groups.keys + fromMessages).distinct()
        return ids.map { id ->
            val isGroup = groups.containsKey(id)
            ChatTarget(
                id = id,
                title = contacts[id] ?: groups[id] ?: shortId(id),
                isGroup = isGroup,
            )
        }.sortedByDescending { chat ->
            messages.lastOrNull { it.convId == chat.id }?.sentAt ?: 0UL
        }
    }

    // Single sync owner: VaultStore fetches once, UI + background collect.
    // This replaces the old dual fetchNow/waitWakeup loops that raced with
    // SyncService on the same destructive cursor (notification shown, list stale).
    val store = remember(client, vaultDir.absolutePath) {
        val c = client
        if (c == null) {
            null
        } else {
            VaultStores.getOrCreate(vaultDir, c).also { it.client = c }
        }
    }
    BindVaultStore(
        store = store,
        atHome = phase == Phase.Home,
        messages = messages,
        contacts = contacts,
        groups = groups,
        outgoing = outgoing,
        outgoingStatus = outgoingStatus,
        selectedId = selected?.id,
        readReceiptsEnabled = readReceipts,
    )

    // Foreground flag for peer read-mark flushing (backgrounding flushes the
    // accumulated viewed max). Desktop stays foreground.
    var isForeground by remember(store) { mutableStateOf(true) }
    LaunchedEffect(store) {
        store?.appForeground?.collect { isForeground = it }
    }

    // Foreground sync is the default but needs the system permission to show
    // anything — ask once on entry instead of failing silently.
    LaunchedEffect(phase) {
        if (phase == Phase.Home && notifyMode == NemoNotifyMode.Foreground) {
            ensureNotifications()
            notificationsAllowed = areNotificationsAllowed()
        }
    }

    LaunchedEffect(showSettings) {
        notificationsAllowed = areNotificationsAllowed()
    }

    Surface(modifier) {
        Box(Modifier.fillMaxSize().imePadding()) {
            AnimatedContent(
                targetState = phase,
                modifier = Modifier.fillMaxSize(),
                transitionSpec = {
                    val forward = phaseOrder(targetState) >= phaseOrder(initialState)
                    val slide = SCREEN_SLIDE_MS
                    val fade = SCREEN_FADE_MS
                    val transform = if (forward) {
                        (
                            slideInHorizontally(tween(slide, easing = FastOutSlowInEasing)) { it / 6 } +
                                fadeIn(tween(fade))
                            ) togetherWith
                            (
                                slideOutHorizontally(tween(slide, easing = FastOutSlowInEasing)) { -it / 6 } +
                                    fadeOut(tween(fade))
                                )
                    } else {
                        (
                            slideInHorizontally(tween(slide, easing = FastOutSlowInEasing)) { -it / 6 } +
                                fadeIn(tween(fade))
                            ) togetherWith
                            (
                                slideOutHorizontally(tween(slide, easing = FastOutSlowInEasing)) { it / 6 } +
                                    fadeOut(tween(fade))
                                )
                    }
                    transform.using(SizeTransform(clip = false)).apply { targetContentZIndex = 1f }
                },
                label = "phase",
            ) { targetPhase ->
                Column(Modifier.fillMaxSize()) {
                    when (targetPhase) {
                        Phase.Create -> {
                            val createIdentity: () -> Unit = {
                                if (!busy) {
                                    runIo {
                                        if (passphrase.length < 8) {
                                            throw IllegalArgumentException("Passphrase must be at least 8 characters")
                                        }
                                        if (passphrase != confirm) {
                                            throw IllegalArgumentException("Passphrases do not match")
                                        }
                                        val c = withContext(Dispatchers.IO) {
                                            vaultDir.mkdirs()
                                            NemoClient.createAt(
                                                vaultDir.absolutePath,
                                                passphrase,
                                                deviceVaultSecret(vaultDir.absolutePath),
                                            )
                                        }
                                        mnemonic = withContext(Dispatchers.IO) { c.takeRevocationMnemonic() }
                                        client = c
                                        VaultStores.getOrCreate(vaultDir, c).also { it.client = c }
                                        publishClient(c)
                                        phase = Phase.Mnemonic
                                    }
                                }
                            }
                            val hasVault = File(vaultDir, "kdf.cbor").isFile
                            OnboardScaffold(
                                headline = "Create identity",
                                footnote = CREATE_FOOTNOTE,
                                footer = if (hasVault) {
                                    {
                                        TextButton(onClick = { phase = Phase.Locked }) {
                                            Text("Unlock existing instead")
                                        }
                                    }
                                } else {
                                    null
                                },
                            ) {
                                PassField(
                                    label = "Passphrase (min 8)",
                                    value = passphrase,
                                    onChange = { passphrase = it },
                                )
                                Spacer(Modifier.height(10.dp))
                                PassField(
                                    label = "Confirm",
                                    value = confirm,
                                    onChange = { confirm = it },
                                    onSubmit = createIdentity,
                                )
                                Spacer(Modifier.height(20.dp))
                                Button(
                                    enabled = !busy,
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .height(52.dp)
                                        .testTag("create-identity"),
                                    onClick = createIdentity,
                                    shape = RoundedCornerShape(14.dp),
                                ) {
                                    if (busy) {
                                        CircularProgressIndicator(Modifier.size(22.dp))
                                    } else {
                                        Text("Create identity")
                                    }
                                }
                            }
                        }
                        Phase.Locked -> {
                            val unlock: () -> Unit = {
                                if (!busy) {
                                    runIo {
                                        val c = withContext(Dispatchers.IO) {
                                            NemoClient.openAt(
                                                vaultDir.absolutePath,
                                                passphrase,
                                                deviceVaultSecret(vaultDir.absolutePath),
                                            )
                                        }
                                        client = c
                                        // Register the single sync owner before publishing,
                                        // so SyncService never sees a client without a store.
                                        VaultStores.getOrCreate(vaultDir, c).also { it.client = c }
                                        publishClient(c)
                                        fingerprint = withContext(Dispatchers.IO) { c.fingerprint() }
                                        identityHex = withContext(Dispatchers.IO) { c.identityIdHex() }
                                        withContext(Dispatchers.IO) { reloadRoster(c) }
                                        val inbox = withContext(Dispatchers.IO) { c.inbox() }
                                        applyIncoming(
                                            messages,
                                            inbox,
                                            outgoing,
                                            outgoingStatus,
                                            ackedUpTo = { id ->
                                                runCatching { c.ackedUpto(id) }.getOrDefault(0UL)
                                            },
                                            readUpTo = { id ->
                                                if (!readReceipts) {
                                                    0UL
                                                } else {
                                                    runCatching { c.readUpto(id) }.getOrDefault(0UL)
                                                }
                                            },
                                        )
                                        VaultStores.findByClient(c)
                                            ?.seed(inbox, contacts.toMap(), groups.toMap())
                                        loadUnreadAfterInbox(vaultDir, messages, lastRead)
                                        // A never-connected vault has no privacy mode yet;
                                        // that must not block unlock — it just means the
                                        // home server step is still pending.
                                        val privacy = withContext(Dispatchers.IO) {
                                            runCatching { c.privacyMode() }
                                        }
                                        privacyMode = privacy.getOrDefault("normal")
                                        registered = privacy.isSuccess
                                        phase = Phase.Home
                                    }
                                }
                            }
                            OnboardScaffold(
                                headline = "Welcome back",
                                footnote = UNLOCK_FOOTNOTE,
                                footer = {
                                    TextButton(onClick = { confirmWipe = true }) {
                                        Text("Create a new identity")
                                    }
                                },
                            ) {
                                PassField(
                                    label = "Passphrase",
                                    value = passphrase,
                                    onChange = { passphrase = it },
                                    onSubmit = unlock,
                                )
                                Spacer(Modifier.height(20.dp))
                                Button(
                                    enabled = !busy,
                                    modifier = Modifier.fillMaxWidth().height(52.dp),
                                    onClick = unlock,
                                    shape = RoundedCornerShape(14.dp),
                                ) {
                                    if (busy) {
                                        CircularProgressIndicator(Modifier.size(22.dp))
                                    } else {
                                        Text("Unlock")
                                    }
                                }
                            }
                            if (confirmWipe) {
                                AlertDialog(
                                    onDismissRequest = { confirmWipe = false },
                                    title = { Text("Replace this identity?") },
                                    text = {
                                        Text(
                                            "This deletes the keys and history on this pane permanently. " +
                                                "There is no recovery. Contacts must add the new identity again.",
                                        )
                                    },
                                    confirmButton = {
                                        TextButton(
                                            onClick = {
                                                confirmWipe = false
                                                wipeVaultDir(vaultDir)
                                                VaultStores.remove(vaultDir)
                                                client = null
                                                publishClient(null)
                                                mnemonic = null
                                                fingerprint = ""
                                                identityHex = ""
                                                registered = false
                                                shareUri = ""
                                                messages.clear()
                                                contacts.clear()
                                                groups.clear()
                                                outgoing.clear()
                                                outgoingStatus.clear()
                                                lastRead.clear()
                                                passphrase = ""
                                                confirm = ""
                                                phase = Phase.Create
                                            },
                                        ) { Text("Delete and create new") }
                                    },
                                    dismissButton = {
                                        TextButton(onClick = { confirmWipe = false }) { Text("Cancel") }
                                    },
                                )
                            }
                        }
                        Phase.Mnemonic -> {
                            val words = (mnemonic ?: "").trim().split(Regex("\\s+")).filter { it.isNotEmpty() }
                            val chipBg = MaterialTheme.colorScheme.surface.copy(
                                alpha = if (nemoDarkTheme()) 0.55f else 0.72f,
                            )
                            OnboardScaffold(
                                headline = "Revocation phrase",
                                footnote = "Write it down offline. It revokes this identity — it cannot unlock or restore anything.",
                            ) {
                                SelectionContainer {
                                    Column(
                                        Modifier.fillMaxWidth(),
                                        verticalArrangement = Arrangement.spacedBy(4.dp),
                                    ) {
                                        words.chunked(3).forEachIndexed { rowIdx, rowWords ->
                                            Row(
                                                Modifier.fillMaxWidth(),
                                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                                            ) {
                                                rowWords.forEachIndexed { colIdx, word ->
                                                    val index = rowIdx * 3 + colIdx + 1
                                                    // Fixed height + fixed-width number slot: every chip
                                                    // measures the same, so long words can never
                                                    // stagger the row (see the wrapped "business" chip).
                                                    Row(
                                                        modifier = Modifier
                                                            .weight(1f)
                                                            .height(34.dp)
                                                            .clip(RoundedCornerShape(10.dp))
                                                            .background(chipBg)
                                                            .padding(horizontal = 6.dp),
                                                        verticalAlignment = Alignment.CenterVertically,
                                                    ) {
                                                        Text(
                                                            "$index",
                                                            style = MaterialTheme.typography.bodySmall,
                                                            fontFamily = FontFamily.Monospace,
                                                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                                                            textAlign = TextAlign.End,
                                                            maxLines = 1,
                                                            modifier = Modifier.width(16.dp),
                                                        )
                                                        Spacer(Modifier.width(6.dp))
                                                        Text(
                                                            word,
                                                            style = MaterialTheme.typography.bodySmall,
                                                            fontFamily = FontFamily.Monospace,
                                                            color = MaterialTheme.colorScheme.onSurface,
                                                            maxLines = 1,
                                                            softWrap = false,
                                                            overflow = TextOverflow.Ellipsis,
                                                            modifier = Modifier.weight(1f),
                                                        )
                                                    }
                                                }
                                                repeat(3 - rowWords.size) {
                                                    Spacer(Modifier.weight(1f))
                                                }
                                            }
                                        }
                                    }
                                }
                                Spacer(Modifier.height(8.dp))
                                TextButton(
                                    modifier = Modifier.fillMaxWidth(),
                                    onClick = {
                                        val phrase = mnemonic.orEmpty()
                                        if (phrase.isNotEmpty()) {
                                            copyToClipboard(phrase)
                                            scope.launch { snackbar.showSnackbar("Copied") }
                                        }
                                    },
                                ) { Text("Copy phrase") }
                                Spacer(Modifier.height(4.dp))
                                Button(
                                    modifier = Modifier.fillMaxWidth().height(52.dp),
                                    onClick = {
                                        runIo {
                                            val c = client ?: return@runIo
                                            fingerprint = withContext(Dispatchers.IO) { c.fingerprint() }
                                            identityHex = withContext(Dispatchers.IO) { c.identityIdHex() }
                                            showSettings = true
                                            phase = Phase.Home
                                        }
                                    },
                                    shape = RoundedCornerShape(14.dp),
                                ) { Text("I wrote it down") }
                            }
                        }
                        Phase.Home -> {
                            val c = client
                            val pickFile = rememberPickFile { path ->
                                val chat = selected ?: return@rememberPickFile
                                attachFilePath(
                                    c,
                                    chat,
                                    path,
                                    messages,
                                    outgoing,
                                    outgoingStatus,
                                    snackbar,
                                    scope,
                                )
                            }
                            val saveFile = rememberSaveFile { ok ->
                                if (ok) {
                                    scope.launch { snackbar.showSnackbar("Saved") }
                                }
                            }
                            // Settings side-effects are hoisted here so the AnimatedContent below
                            // stays pure (share-url minting must not replay for both children
                            // mid-transition). No point minting before first connect.
                            LaunchedEffect(showSettings, registered) {
                                if (showSettings && registered) {
                                    runIo {
                                        shareUri = withContext(Dispatchers.IO) {
                                            c?.mintShareUri().orEmpty()
                                        }
                                    }
                                }
                            }
                            LaunchedEffect(showSettings, selected?.id) {
                                val cur = selected
                                if (showSettings && cur != null && !cur.isGroup) {
                                    contactNickname = contacts[cur.id] ?: cur.title
                                }
                            }
                            val runIoFn: (suspend () -> Unit) -> Unit = { block -> runIo(block) }
                            val markOutgoingFn: (DisplayRow) -> Unit = { markOutgoing(it) }
                            BoxWithConstraints(Modifier.fillMaxSize()) {
                                val split = maxWidth >= 720.dp
                                val chat = selected
                                val chatList = chats()
                                // System / edge-swipe back: settings -> chat list -> leave app.
                                NemoBackHandler(enabled = showSettings || selected != null) {
                                    when {
                                        showSettings -> showSettings = false
                                        selected != null -> selected = null
                                    }
                                }
                                AnimatedContent(
                                    targetState = showSettings,
                                    modifier = Modifier.fillMaxSize(),
                                    transitionSpec = {
                                        val slide = SCREEN_SLIDE_MS
                                        val fade = SCREEN_FADE_MS
                                        val transform = if (targetState) {
                                            // Entering settings: new slides from right, old exits left.
                                            (
                                                slideInHorizontally(tween(slide, easing = FastOutSlowInEasing)) { it / 4 } +
                                                    fadeIn(tween(fade))
                                                ) togetherWith
                                                (
                                                    slideOutHorizontally(tween(slide, easing = FastOutSlowInEasing)) { -it / 8 } +
                                                        fadeOut(tween(fade))
                                                    )
                                        } else {
                                            (
                                                slideInHorizontally(tween(slide, easing = FastOutSlowInEasing)) { -it / 8 } +
                                                    fadeIn(tween(fade))
                                                ) togetherWith
                                                (
                                                    slideOutHorizontally(tween(slide, easing = FastOutSlowInEasing)) { it / 4 } +
                                                        fadeOut(tween(fade))
                                                    )
                                        }
                                        transform.using(SizeTransform(clip = false)).apply { targetContentZIndex = 1f }
                                    },
                                    label = "settings",
                                ) { settingsVisible ->
                                    if (settingsVisible) {
                                        SettingsScreen(
                                            fingerprint = fingerprint,
                                            identityHex = identityHex,
                                            homeUrl = homeUrl,
                                            onHomeUrl = { homeUrl = it },
                                            shareUri = shareUri,
                                            joinUri = joinUri,
                                            joinPaste = joinPaste,
                                            onJoinPaste = { joinPaste = it },
                                            inviteUri = inviteUri,
                                            inviteSendTo = inviteSendTo,
                                            onInviteSendTo = { inviteSendTo = it },
                                            roster = contacts.toMap(),
                                            onSendInvite = {
                                                runIo {
                                                    val target = inviteSendTo
                                                    val text = inviteUri
                                                    if (target.isBlank() || text.isBlank()) return@runIo
                                                    val row = withContext(Dispatchers.IO) {
                                                        c?.sendText(target, text)
                                                    } ?: return@runIo
                                                    messages.add(row)
                                                    markOutgoing(row)
                                                    inviteUri = ""
                                                    inviteSendTo = ""
                                                    showSettings = false
                                                    selected = ChatTarget(target, contacts[target] ?: shortId(target), false)
                                                    snackbar.showSnackbar("Invite sent — they accept it in the chat")
                                                }
                                            },
                                            memberCred = memberCred,
                                            disappearSecs = disappearSecs,
                                            onDisappearSecs = { disappearSecs = it },
                                            contactNickname = contactNickname,
                                            onContactNickname = { contactNickname = it },
                                            selected = chat,
                                            busy = busy,
                                            onBack = { showSettings = false },
                                            onRegister = {
                                                runIo {
                                                    val moving = registered
                                                    try {
                                                        withContext(Dispatchers.IO) { c?.register(homeUrl.trim()) }
                                                    } catch (e: Throwable) {
                                                        // Same URL on an already-connected client is
                                                        // not a failure — just confirm the state.
                                                        if (!isAlreadyRegisteredError(e)) throw e
                                                        registered = true
                                                        snackbar.showSnackbar("Already connected to this home server.")
                                                        return@runIo
                                                    }
                                                    registered = true
                                                    snackbar.showSnackbar(
                                                        if (moving) "Moved to new home" else "Connected to home",
                                                    )
                                                }
                                            },
                                            onShare = {
                                                runIo {
                                                    shareUri = withContext(Dispatchers.IO) {
                                                        c?.mintShareUri().orEmpty()
                                                    }
                                                    if (shareUri.isNotEmpty()) copyToClipboard(shareUri)
                                                }
                                            },
                                            onAdmit = {
                                                runIo {
                                                    memberCred = withContext(Dispatchers.IO) {
                                                        c?.admitJoin(joinPaste.trim()).orEmpty()
                                                    }
                                                    snackbar.showSnackbar("Admitted member")
                                                }
                                            },
                                            onInviteGroup = {
                                                val id = chat?.id ?: return@SettingsScreen
                                                if (chat.isGroup.not()) return@SettingsScreen
                                                runIo {
                                                    val uri = withContext(Dispatchers.IO) {
                                                        c?.mintGroupInvite(id).orEmpty()
                                                    }
                                                    inviteUri = uri
                                                    copyToClipboard(uri)
                                                    snackbar.showSnackbar("Invite copied — or send it via 1:1 below")
                                                }
                                            },
                                            onSaveNickname = {
                                                val id = chat?.id ?: return@SettingsScreen
                                                if (chat.isGroup) return@SettingsScreen
                                                runIo {
                                                    val name = contactNickname.trim()
                                                    withContext(Dispatchers.IO) {
                                                        c?.setNickname(id, name)
                                                    }
                                                    if (name.isEmpty()) {
                                                        contacts.remove(id)
                                                    } else {
                                                        contacts[id] = name
                                                    }
                                                    selected = chat.copy(title = name.ifBlank { shortId(id) })
                                                    snackbar.showSnackbar("Contact name saved")
                                                }
                                            },
                                            onDisappear = {
                                                val id = chat?.id ?: return@SettingsScreen
                                                runIo {
                                                    val secs = disappearSecs.toULongOrNull() ?: 0UL
                                                    val row = withContext(Dispatchers.IO) {
                                                        c?.setDisappear(id, secs)
                                                    } ?: return@runIo
                                                    messages.add(row)
                                                    markOutgoing(row)
                                                }
                                            },
                                            privacyMode = privacyMode,
                                            onPrivacyMode = { mode ->
                                                runIo {
                                                    withContext(Dispatchers.IO) {
                                                        c?.setPrivacyMode(mode)
                                                    }
                                                    privacyMode = mode
                                                }
                                            },
                                            readReceiptsEnabled = readReceipts,
                                            onReadReceipts = { enabled ->
                                                saveReadReceiptsEnabled(enabled)
                                                readReceipts = enabled
                                            },
                                            notifyMode = notifyMode,
                                            onNotifyMode = { mode ->
                                                pendingNotify = mode
                                                ensureNotifications()
                                            },
                                            notificationsAllowed = notificationsAllowed,
                                            onOpenSystemSettings = { openSystemNotificationSettings() },
                                        )
                                    } else if (split) {
                                        Row(Modifier.fillMaxSize()) {
                                            ChatListPane(
                                                label = label,
                                                chats = chatList,
                                                messages = messages,
                                                lastRead = lastRead,
                                                selectedId = chat?.id,
                                                modifier = Modifier.width(340.dp).fillMaxHeight(),
                                                onSelect = {
                                                    selected = it
                                                    showSettings = false
                                                },
                                                onSettings = { showSettings = true },
                                                onAdd = { addMenu = true },
                                                addMenu = addMenu,
                                                onAddDismiss = { addMenu = false },
                                                onAddContact = {
                                                    addMenu = false
                                                    sheet = Sheet.AddContact
                                                },
                                                onNewGroup = {
                                                    addMenu = false
                                                    sheet = Sheet.NewGroup
                                                },
                                                onJoinGroup = {
                                                    addMenu = false
                                                    sheet = Sheet.JoinGroup
                                                },
                                                showConnectBanner = !registered,
                                                onConnect = { showSettings = true },
                                            )
                                            VerticalDivider()
                                            Box(Modifier.weight(1f).fillMaxHeight()) {
                                                // Split pane: crossfade only, the list never moves.
                                                Crossfade(
                                                    targetState = chat,
                                                    animationSpec = tween(THREAD_FADE_MS),
                                                    label = "thread",
                                                    modifier = Modifier.fillMaxSize(),
                                                ) { animated ->
                                                    if (animated == null) {
                                                        EmptyChatHint()
                                                    } else {
                                                        ActiveChatThread(
                                                            c = c,
                                                            target = animated,
                                                            messages = messages,
                                                            outgoing = outgoing,
                                                            outgoingStatus = outgoingStatus,
                                                            draft = draft,
                                                            onDraft = { draft = it },
                                                            showBack = false,
                                                            onBack = { selected = null },
                                                            onSettings = { showSettings = true },
                                                            chatMenu = chatMenu,
                                                            onChatMenu = { chatMenu = it },
                                                            scope = scope,
                                                            snackbar = snackbar,
                                                            runIo = runIoFn,
                                                            onMarkOutgoing = markOutgoingFn,
                                                            saveFile = saveFile,
                                                            lastRead = lastRead,
                                                            vaultDir = vaultDir,
                                                            readReceiptsEnabled = readReceipts,
                                                            isForeground = isForeground,
                                                            acceptedInvites = acceptedInvites,
                                                            admittedJoins = admittedJoins,
                                                            onAcceptFallback = ::acceptFallback,
                                                            contacts = contacts,
                                                        )
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        // Phone: push navigation, thread slides from right.
                                        AnimatedContent(
                                            targetState = chat,
                                            modifier = Modifier.fillMaxSize(),
                                            transitionSpec = {
                                                val slide = SCREEN_SLIDE_MS
                                                val fade = SCREEN_FADE_MS
                                                val transform = if (targetState != null) {
                                                    (
                                                        slideInHorizontally(tween(slide, easing = FastOutSlowInEasing)) { it / 4 } +
                                                            fadeIn(tween(fade))
                                                        ) togetherWith
                                                        (
                                                            slideOutHorizontally(tween(slide, easing = FastOutSlowInEasing)) { -it / 8 } +
                                                                fadeOut(tween(fade))
                                                            )
                                                } else {
                                                    (
                                                        slideInHorizontally(tween(slide, easing = FastOutSlowInEasing)) { -it / 8 } +
                                                            fadeIn(tween(fade))
                                                        ) togetherWith
                                                        (
                                                            slideOutHorizontally(tween(slide, easing = FastOutSlowInEasing)) { it / 4 } +
                                                                fadeOut(tween(fade))
                                                            )
                                                }
                                                transform.using(SizeTransform(clip = false)).apply { targetContentZIndex = 1f }
                                            },
                                            label = "chat-push",
                                        ) { animated ->
                                            if (animated == null) {
                                                ChatListPane(
                                                    label = label,
                                                    chats = chatList,
                                                    messages = messages,
                                                    lastRead = lastRead,
                                                    selectedId = null,
                                                    modifier = Modifier.fillMaxSize(),
                                                    onSelect = { selected = it },
                                                    onSettings = { showSettings = true },
                                                    onAdd = { addMenu = true },
                                                    addMenu = addMenu,
                                                    onAddDismiss = { addMenu = false },
                                                    onAddContact = {
                                                        addMenu = false
                                                        sheet = Sheet.AddContact
                                                    },
                                                    onNewGroup = {
                                                        addMenu = false
                                                        sheet = Sheet.NewGroup
                                                    },
                                                    onJoinGroup = {
                                                        addMenu = false
                                                        sheet = Sheet.JoinGroup
                                                    },
                                                    showConnectBanner = !registered,
                                                    onConnect = { showSettings = true },
                                                )
                                            } else {
                                                ActiveChatThread(
                                                    c = c,
                                                    target = animated,
                                                    messages = messages,
                                                    outgoing = outgoing,
                                                    outgoingStatus = outgoingStatus,
                                                    draft = draft,
                                                    onDraft = { draft = it },
                                                    showBack = true,
                                                    onBack = { selected = null },
                                                    onSettings = { showSettings = true },
                                                    chatMenu = chatMenu,
                                                    onChatMenu = { chatMenu = it },
                                                    scope = scope,
                                                    snackbar = snackbar,
                                                    runIo = runIoFn,
                                                    onMarkOutgoing = markOutgoingFn,
                                                    saveFile = saveFile,
                                                    lastRead = lastRead,
                                                    vaultDir = vaultDir,
                                                    readReceiptsEnabled = readReceipts,
                                                    isForeground = isForeground,
                                                    acceptedInvites = acceptedInvites,
                                                    admittedJoins = admittedJoins,
                                                    onAcceptFallback = ::acceptFallback,
                                                    contacts = contacts,
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                            if (sheet != Sheet.None) {
                                ModalBottomSheet(
                                    onDismissRequest = { sheet = Sheet.None },
                                    sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
                                ) {
                                    when (sheet) {
                                        Sheet.AddContact -> {
                                            AddContactSheet(
                                                client = c,
                                                busy = busy,
                                                onAdd = { rawCard, name ->
                                                    runIo {
                                                        val peer = withContext(Dispatchers.IO) {
                                                            c?.addContact(
                                                                rawCard.trim(),
                                                                name.ifBlank { "Contact" },
                                                            ).orEmpty()
                                                        }
                                                        contacts[peer] = name.ifBlank { shortId(peer) }
                                                        selected = ChatTarget(peer, contacts[peer] ?: shortId(peer), false)
                                                        sheet = Sheet.None
                                                    }
                                                },
                                            )
                                        }
                                        Sheet.NewGroup -> SheetForm(
                                            title = "New group",
                                            action = "Create",
                                            enabled = !busy && groupName.isNotBlank(),
                                            onAction = {
                                                runIo {
                                                    val id = withContext(Dispatchers.IO) {
                                                        c?.createGroup(groupName).orEmpty()
                                                    }
                                                    groups[id] = groupName
                                                    selected = ChatTarget(id, groupName, true)
                                                    groupName = ""
                                                    sheet = Sheet.None
                                                }
                                            },
                                        ) {
                                            OutlinedTextField(
                                                value = groupName,
                                                onValueChange = { groupName = it },
                                                label = { Text("Group name") },
                                                modifier = Modifier.fillMaxWidth(),
                                                singleLine = true,
                                            )
                                        }
                                        Sheet.JoinGroup -> SheetForm(
                                            title = "Join group",
                                            action = "Accept invite",
                                            enabled = !busy && invitePaste.isNotBlank(),
                                            onAction = {
                                                runIo {
                                                    joinUri = withContext(Dispatchers.IO) {
                                                        c?.acceptGroupInvite(invitePaste.trim()).orEmpty()
                                                    }
                                                    copyToClipboard(joinUri)
                                                    invitePaste = ""
                                                    snackbar.showSnackbar("Join request ready — send via 1:1 or copy it manually")
                                                }
                                            },
                                        ) {
                                            OutlinedTextField(
                                                value = invitePaste,
                                                onValueChange = { invitePaste = it },
                                                label = { Text("Group invite") },
                                                placeholder = { Text("nemo-g:1:…") },
                                                supportingText = { Text("Paste the invite a group member shared.") },
                                                modifier = Modifier.fillMaxWidth(),
                                                singleLine = true,
                                                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                                            )
                                            if (joinUri.isNotEmpty()) {
                                                RelaySendPicker(
                                                    header = "Join request ready (also copied). Send it directly:",
                                                    roster = contacts,
                                                    selectedId = joinSendTo,
                                                    onSelect = { joinSendTo = it },
                                                    contactPlaceholder = "Pick a group member",
                                                    sendLabel = "Send join request via 1:1",
                                                    sendEnabled = !busy && joinSendTo.isNotBlank(),
                                                    onSend = {
                                                        runIo {
                                                            val target = joinSendTo
                                                            val row = withContext(Dispatchers.IO) {
                                                                c?.sendText(target, joinUri)
                                                            } ?: return@runIo
                                                            messages.add(row)
                                                            markOutgoing(row)
                                                            joinUri = ""
                                                            joinSendTo = ""
                                                            sheet = Sheet.None
                                                            snackbar.showSnackbar("Join request sent")
                                                        }
                                                    },
                                                )
                                            }
                                        }
                                        Sheet.None -> {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
            SnackbarHost(
                hostState = snackbar,
                modifier = Modifier
                    .align(Alignment.TopCenter)
                    .statusBarsPadding()
                    .padding(horizontal = 12.dp, vertical = 8.dp),
            )
        }
    }
}

private fun sendChat(
    client: NemoClient?,
    chat: ChatTarget,
    draft: String,
    messages: MutableList<DisplayRow>,
    outgoing: MutableMap<String, Boolean>,
    outgoingStatus: MutableMap<String, OutgoingStatus>,
    scope: CoroutineScope,
    snackbar: SnackbarHostState,
    clear: () -> Unit,
    replyTo: ULong = 0UL,
) {
    val text = draft.trim()
    if (text.isEmpty()) return
    if (isOverTextLimit(text)) {
        scope.launch {
            snackbar.showSnackbar("Message is too long — keep it under 8192 bytes.")
        }
        return
    }
    clear()
    val localId = newLocalId()
    val pending = optimisticTextRow(chat.id, text, localId, replyTo)
    messages.add(pending)
    outgoing[localId] = true
    outgoingStatus[localId] = OutgoingStatus.Pending
    scope.launch {
        try {
            val row = withContext(Dispatchers.IO) {
                if (chat.isGroup) {
                    if (replyTo != 0UL) {
                        client?.sendGroupTextWithReply(chat.id, text, replyTo)
                    } else {
                        client?.sendGroupText(chat.id, text)
                    }
                } else {
                    if (replyTo != 0UL) {
                        client?.sendTextWithReply(chat.id, text, replyTo)
                    } else {
                        client?.sendText(chat.id, text)
                    }
                }
            } ?: throw IllegalStateException("Send failed")
            val idx = messages.indexOfFirst { it.fetchToken == localId }
            if (idx >= 0) {
                messages[idx] = row.copy(fetchToken = localId)
            }
            // Drop a duplicate if fetch raced in the same seq without the local key.
            val dupes = messages.withIndex().filter { (i, m) ->
                i != idx &&
                    m.convId == row.convId &&
                    m.convSeq == row.convSeq &&
                    m.kind == row.kind &&
                    !m.fetchToken.startsWith("local:")
            }.map { it.index }.sortedDescending()
            for (i in dupes) messages.removeAt(i)
            // Network send finished: single ✓ (Sent). The second tick (✓✓)
            // arrives only via a real peer protocol_ack (see applyIncoming).
            outgoingStatus[localId] = OutgoingStatus.Sent
        } catch (e: Throwable) {
            outgoingStatus[localId] = OutgoingStatus.Failed
            snackbar.showSnackbar(e.message ?: e.toString())
        }
    }
}

private fun attachFilePath(
    client: NemoClient?,
    chat: ChatTarget,
    path: String,
    messages: MutableList<DisplayRow>,
    outgoing: MutableMap<String, Boolean>,
    outgoingStatus: MutableMap<String, OutgoingStatus>,
    snackbar: SnackbarHostState,
    scope: CoroutineScope,
) {
    val f = File(path)
    if (!f.isFile) {
        scope.launch { snackbar.showSnackbar("File not found") }
        return
    }
    val bytes = try {
        f.readBytes()
    } catch (e: Throwable) {
        scope.launch { snackbar.showSnackbar(e.message ?: e.toString()) }
        return
    }
    val localId = newLocalId()
    val pending = optimisticFileRow(chat.id, f.name, bytes, localId)
    messages.add(pending)
    outgoing[localId] = true
    outgoingStatus[localId] = OutgoingStatus.Pending
    scope.launch {
        try {
            val row = withContext(Dispatchers.IO) {
                if (chat.isGroup) {
                    client?.sendGroupFile(chat.id, f.name, "application/octet-stream", bytes)
                } else {
                    client?.sendFile(chat.id, f.name, "application/octet-stream", bytes)
                }
            } ?: throw IllegalStateException("Send failed")
            val idx = messages.indexOfFirst { it.fetchToken == localId }
            if (idx >= 0) {
                messages[idx] = row.copy(fetchToken = localId)
            }
            val dupes = messages.withIndex().filter { (i, m) ->
                i != idx &&
                    m.convId == row.convId &&
                    m.convSeq == row.convSeq &&
                    m.kind == row.kind &&
                    !m.fetchToken.startsWith("local:")
            }.map { it.index }.sortedDescending()
            for (i in dupes) messages.removeAt(i)
            // Single ✓ (Sent); ✓✓ comes only via peer protocol_ack.
            outgoingStatus[localId] = OutgoingStatus.Sent
            snackbar.showSnackbar("Sent ${f.name}")
        } catch (e: Throwable) {
            outgoingStatus[localId] = OutgoingStatus.Failed
            snackbar.showSnackbar(e.message ?: e.toString())
        }
    }
}

@Composable
private fun OnboardScaffold(
    headline: String,
    footnote: String? = null,
    footer: (@Composable () -> Unit)? = null,
    content: @Composable () -> Unit,
) {
    val atmosphere = Brush.verticalGradient(colors = LocalNemoPalette.current.onboardGradient)
    val brandEnter = remember { Animatable(0f) }
    LaunchedEffect(Unit) {
        brandEnter.animateTo(
            targetValue = 1f,
            animationSpec = tween(durationMillis = 420, easing = FastOutSlowInEasing),
        )
    }
    val t = brandEnter.value
    Box(
        Modifier
            .fillMaxSize()
            .background(atmosphere)
            .statusBarsPadding()
            .navigationBarsPadding(),
    ) {
        Column(
            Modifier
                .fillMaxSize()
                .widthIn(max = 420.dp)
                .align(Alignment.TopCenter)
                .padding(horizontal = 28.dp, vertical = 20.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Spacer(Modifier.height(28.dp))
            Column(
                Modifier.graphicsLayer {
                    alpha = t
                    scaleX = 0.92f + 0.08f * t
                    scaleY = 0.92f + 0.08f * t
                },
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                NemoBrandMark(Modifier.size(112.dp))
                Spacer(Modifier.height(14.dp))
                Text(
                    "Nemo",
                    style = MaterialTheme.typography.headlineLarge,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onBackground,
                )
            }
            Spacer(Modifier.height(36.dp))
            Text(
                headline,
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.SemiBold,
                textAlign = TextAlign.Center,
                color = MaterialTheme.colorScheme.onBackground,
            )
            Spacer(Modifier.height(18.dp))
            content()
            if (!footnote.isNullOrBlank()) {
                Spacer(Modifier.height(14.dp))
                Text(
                    footnote,
                    style = MaterialTheme.typography.bodySmall,
                    textAlign = TextAlign.Center,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.9f),
                    modifier = Modifier.padding(horizontal = 8.dp),
                )
            }
            Spacer(Modifier.weight(1f))
            if (footer != null) {
                footer()
                Spacer(Modifier.height(8.dp))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ChatListPane(
    label: String,
    chats: List<ChatTarget>,
    messages: List<DisplayRow>,
    selectedId: String?,
    modifier: Modifier,
    onSelect: (ChatTarget) -> Unit,
    onSettings: () -> Unit,
    onAdd: () -> Unit,
    addMenu: Boolean,
    onAddDismiss: () -> Unit,
    onAddContact: () -> Unit,
    onNewGroup: () -> Unit,
    onJoinGroup: () -> Unit,
    showConnectBanner: Boolean = false,
    onConnect: () -> Unit = {},
    lastRead: Map<String, ULong> = emptyMap(),
) {
    val palette = LocalNemoPalette.current
    val listBg = if (palette.dark) palette.chat else palette.list
    Scaffold(
        modifier = modifier,
        containerColor = listBg,
        topBar = {
            CenterAlignedTopAppBar(
                navigationIcon = {
                    NemoBrandMark(
                        Modifier
                            .padding(start = 12.dp)
                            .size(28.dp),
                    )
                },
                title = {
                    Column(horizontalAlignment = Alignment.CenterHorizontally) {
                        Text("Chats", fontWeight = FontWeight.SemiBold)
                        if (label != "Nemo") {
                            Text(
                                label,
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = listBg,
                    scrolledContainerColor = listBg,
                ),
                actions = {
                    IconButton(onClick = onSettings) {
                        Icon(Icons.Filled.Settings, contentDescription = "Settings")
                    }
                },
            )
        },
        floatingActionButton = {
            Box {
                FloatingActionButton(
                    onClick = onAdd,
                    containerColor = MaterialTheme.colorScheme.primary,
                    contentColor = MaterialTheme.colorScheme.onPrimary,
                ) {
                    Icon(Icons.Filled.Add, contentDescription = "New chat")
                }
                DropdownMenu(
                    expanded = addMenu,
                    onDismissRequest = onAddDismiss,
                    containerColor = MaterialTheme.colorScheme.surface,
                    tonalElevation = 0.dp,
                    shadowElevation = 8.dp,
                ) {
                    DropdownMenuItem(
                        text = { Text("New chat") },
                        leadingIcon = {
                            Icon(Icons.Filled.PersonAdd, null, tint = MaterialTheme.colorScheme.primary)
                        },
                        onClick = onAddContact,
                        colors = MenuDefaults.itemColors(
                            leadingIconColor = MaterialTheme.colorScheme.primary,
                        ),
                    )
                    DropdownMenuItem(
                        text = { Text("New group") },
                        leadingIcon = {
                            Icon(Icons.Filled.Group, null, tint = MaterialTheme.colorScheme.primary)
                        },
                        onClick = onNewGroup,
                        colors = MenuDefaults.itemColors(
                            leadingIconColor = MaterialTheme.colorScheme.primary,
                        ),
                    )
                    DropdownMenuItem(
                        text = { Text("Join group") },
                        leadingIcon = {
                            Icon(Icons.AutoMirrored.Filled.Chat, null, tint = MaterialTheme.colorScheme.primary)
                        },
                        onClick = onJoinGroup,
                        colors = MenuDefaults.itemColors(
                            leadingIconColor = MaterialTheme.colorScheme.primary,
                        ),
                    )
                }
            }
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding)) {
            if (showConnectBanner) {
                NotConnectedBanner(
                    onConnect = onConnect,
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
            if (chats.isEmpty()) {
                Column(
                    Modifier
                        .weight(1f)
                        .fillMaxWidth()
                        .padding(horizontal = 32.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.Center,
                ) {
                    NemoBrandMark(Modifier.size(72.dp))
                    Spacer(Modifier.height(16.dp))
                    Text(
                        "No conversations yet",
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.SemiBold,
                        textAlign = TextAlign.Center,
                    )
                    Spacer(Modifier.height(8.dp))
                    Text(
                        "Add someone with a contact card to start.",
                        style = MaterialTheme.typography.bodyMedium,
                        textAlign = TextAlign.Center,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.height(24.dp))
                    Button(
                        onClick = onAdd,
                        modifier = Modifier.height(48.dp),
                        shape = RoundedCornerShape(14.dp),
                    ) { Text("New chat") }
                }
            } else {
                LazyColumn(Modifier.weight(1f).fillMaxWidth()) {
                    items(chats, key = { it.id }) { chat ->
                        val last = lastBubble(messages, chat.id)
                        ChatRow(
                            chat = chat,
                            preview = last?.let { previewLine(it) } ?: "No messages yet",
                            time = last?.let { formatTime(it.sentAt) }.orEmpty(),
                            selected = chat.id == selectedId,
                            unread = unreadCount(chat.id, messages, lastRead),
                            onClick = { onSelect(chat) },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun NotConnectedBanner(onConnect: () -> Unit, modifier: Modifier = Modifier) {
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(14.dp),
        color = MaterialTheme.colorScheme.primaryContainer,
    ) {
        Row(
            Modifier.padding(horizontal = 14.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                Icons.Filled.CloudOff,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onPrimaryContainer,
            )
            Spacer(Modifier.width(12.dp))
            Column(Modifier.weight(1f)) {
                Text(
                    "Not connected",
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                )
                Text(
                    "Connect to a home server to chat.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onPrimaryContainer,
                )
            }
            TextButton(onClick = onConnect) { Text("Connect") }
        }
    }
}

@Composable
private fun ChatRow(chat: ChatTarget, preview: String, time: String, selected: Boolean, onClick: () -> Unit, unread: Int = 0) {
    val bg = when {
        selected -> MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.55f)
        else -> Color.Transparent
    }
    val hairline = MaterialTheme.colorScheme.outline.copy(alpha = 0.35f)
    val hasUnread = unread > 0
    Column(Modifier.fillMaxWidth().background(bg)) {
        Row(
            Modifier
                .fillMaxWidth()
                .clickable(onClick = onClick)
                .padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Avatar(chat.title, chat.isGroup)
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        chat.title,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = if (hasUnread) FontWeight.Bold else null,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    if (time.isNotEmpty()) {
                        Text(
                            time,
                            style = MaterialTheme.typography.labelSmall,
                            color = if (selected || hasUnread) {
                                MaterialTheme.colorScheme.primary
                            } else {
                                MaterialTheme.colorScheme.onSurfaceVariant
                            },
                            fontWeight = if (hasUnread) FontWeight.Bold else null,
                        )
                    }
                }
                Spacer(Modifier.height(2.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        preview,
                        style = MaterialTheme.typography.bodyMedium,
                        color = if (hasUnread) {
                            MaterialTheme.colorScheme.onSurface
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        },
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    if (hasUnread) {
                        Spacer(Modifier.width(8.dp))
                        UnreadBadge(count = unread, convId = chat.id)
                    }
                }
            }
        }
        Box(
            Modifier
                .fillMaxWidth()
                .padding(start = 78.dp)
                .height(1.dp)
                .background(hairline),
        )
    }
}

@Composable
private fun UnreadBadge(count: Int, convId: String) {
    val label = if (count > 99) "99+" else count.toString()
    Box(
        modifier = Modifier
            .heightIn(min = 20.dp)
            .widthIn(min = 20.dp)
            .clip(RoundedCornerShape(10.dp))
            .background(MaterialTheme.colorScheme.primary)
            .padding(horizontal = 7.dp, vertical = 2.dp)
            .testTag("unread-$convId"),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.onPrimary,
            maxLines = 1,
        )
    }
}

@Composable
private fun UnreadMarker() {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp)
            .testTag("unread-marker"),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier
                .weight(1f)
                .height(1.dp)
                .background(MaterialTheme.colorScheme.primary.copy(alpha = 0.5f)),
        )
        Surface(
            shape = RoundedCornerShape(12.dp),
            color = MaterialTheme.colorScheme.primaryContainer,
            modifier = Modifier.padding(horizontal = 8.dp),
        ) {
            Text(
                "Unread messages",
                style = MaterialTheme.typography.labelSmall,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onPrimaryContainer,
                modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
                maxLines = 1,
            )
        }
        Box(
            Modifier
                .weight(1f)
                .height(1.dp)
                .background(MaterialTheme.colorScheme.primary.copy(alpha = 0.5f)),
        )
    }
}

@Composable
private fun JumpLatestButton(count: Int, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Surface(
        onClick = onClick,
        shape = RoundedCornerShape(20.dp),
        color = MaterialTheme.colorScheme.primaryContainer,
        shadowElevation = 4.dp,
        modifier = modifier.testTag("jump-latest"),
    ) {
        Row(
            Modifier.padding(horizontal = 14.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                Icons.Filled.ArrowDownward,
                contentDescription = "Jump to new messages",
                tint = MaterialTheme.colorScheme.onPrimaryContainer,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(6.dp))
            Text(
                if (count > 99) "99+ new" else "$count new",
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onPrimaryContainer,
                maxLines = 1,
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ChatThread(
    chat: ChatTarget,
    messages: List<DisplayRow>,
    outgoing: Map<String, Boolean>,
    outgoingStatus: Map<String, OutgoingStatus>,
    draft: String,
    onDraft: (String) -> Unit,
    showBack: Boolean,
    onBack: () -> Unit,
    onSettings: () -> Unit,
    chatMenu: Boolean,
    onChatMenu: (Boolean) -> Unit,
    onSend: () -> Unit,
    onAttach: () -> Unit,
    onCall: () -> Unit,
    onAnswer: () -> Unit,
    onHangup: () -> Unit,
    onDecline: () -> Unit,
    onReact: (DisplayRow) -> Unit,
    onDelete: (DisplayRow) -> Unit,
    onSave: (DisplayRow) -> Unit,
    entryMark: ULong = 0UL,
    readMark: ULong = 0UL,
    onVisibleRead: (ULong) -> Unit = {},
    onAdmitJoin: ((DisplayRow) -> Unit)? = null,
    onAcceptInvite: ((DisplayRow) -> Unit)? = null,
    acceptedInvites: Set<String> = emptySet(),
    admittedJoins: Set<String> = emptySet(),
    showSenderNames: Boolean = false,
    foreground: Boolean = true,
    replyTo: DisplayRow? = null,
    onClearReply: () -> Unit = {},
    onReply: (DisplayRow) -> Unit = {},
    onPillClick: (DisplayRow, String) -> Unit = { _, _ -> },
    onPillLongClick: (DisplayRow) -> Unit = { _ -> },
    contacts: Map<String, String> = emptyMap(),
    onCopy: (DisplayRow) -> Unit = {},
    onQuickReact: (DisplayRow, String) -> Unit = { _, _ -> },
) {
    val listState = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val palette = LocalNemoPalette.current
    val dark = palette.dark
    val wallpaper = palette.chat
    val draftBytes = draftByteSize(draft)
    val overLimit = draftBytes > TEXT_MAX_BYTES
    val canSend = draft.isNotBlank() && !overLimit
    val sendScale by animateFloatAsState(
        targetValue = if (canSend) 1f else 0.88f,
        animationSpec = spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessMediumLow),
        label = "sendScale",
    )
    val lastKey = messages.lastOrNull()?.let { messageListKey(it) }
    val knownKeys = remember(chat.id) { mutableSetOf<String>() }
    var seedDone by remember(chat.id) { mutableStateOf(false) }
    // Jump target: oldest unread at open, frozen for the visit.
    val entryUnreadSeq = remember(chat.id) { firstUnreadSeq(messages, chat.id, entryMark) }
    // Newest seq present at open: rows arriving later never raise the marker,
    // so it can't flash in and out while watching the bottom of the thread.
    val openMaxSeq = remember(chat.id) { messages.maxOfOrNull { it.convSeq } ?: 0UL }
    // Start unpinned when jumping, so the bottom-pin below can't yank the
    // list down mid-jump.
    var stickToBottom by remember(chat.id) { mutableStateOf(entryUnreadSeq == null) }
    // Marker anchor: oldest unread at open, frozen for the whole visit. It is
    // cleared only on leave (the persisted mark decides the next visit), so
    // scrolling past it never moves or dismisses it mid-read.
    val dividerAt = remember(messages, entryUnreadSeq) {
        val seq = entryUnreadSeq ?: return@remember null
        val chrono = messages.indexOfFirst { it.convId == chat.id && it.convSeq == seq }
        if (chrono < 0) return@remember null
        (messages.lastIndex - chrono) + 1
    }
    var markEnabled by remember(chat.id) { mutableStateOf(false) }
    // True once the user scrolls themselves. Landing viewport rows must not
    // count as seen on open alone; programmatic jumps never set this.
    var userScrolled by remember(chat.id) { mutableStateOf(false) }
    val latestVisibleRead = rememberUpdatedState(onVisibleRead)

    // Floating date lives in its own state holder so the thread keeps its
    // unread-marker indices and visible-index math untouched.
    // autoScrolling marks programmatic animations (pin-to-bottom on send /
    // receive, jump-to-latest) so the date pill — finger scrolling only —
    // ignores them.
    var autoScrolling by remember(chat.id) { mutableStateOf(false) }
    suspend fun pinToBottomAnimated() {
        autoScrolling = true
        try {
            listState.animateChatToBottom(animated = true)
        } finally {
            autoScrolling = false
        }
    }
    val floatingDate = rememberFloatingDateUiState(messages, chat.id, listState, dividerAt, autoScrolling)

    var overlayRoot by remember { mutableStateOf<LayoutCoordinates?>(null) }
    var composerCoords by remember { mutableStateOf<LayoutCoordinates?>(null) }
    var overlayWindowOrigin by remember { mutableStateOf(Offset.Zero) }
    var rootHeightPx by remember { mutableFloatStateOf(1f) }
    var pendingFly by remember(chat.id) { mutableStateOf<PendingMessageSend?>(null) }
    var showMenagerie by remember(chat.id) { mutableStateOf(false) }
    val activeFlies = remember(chat.id) { mutableStateListOf<MessageSendAnimation>() }
    val flyTargets = remember(chat.id) { mutableStateMapOf<String, Rect>() }
    // Platform split: mobile morphs composer → bubble;
    // desktop has no morph — new bubbles just appear (see MessageSendAnimation).
    val useFlyMorph = messageSendFlyMobileFeel()
    val flyingKeys = remember(activeFlies.size, activeFlies.map { it.listKey }) {
        activeFlies.map { it.listKey }.toSet()
    }

    LaunchedEffect(chat.id, messages.size) {
        if (!seedDone) {
            knownKeys.clear()
            knownKeys.addAll(messages.map { messageListKey(it) })
            seedDone = true
        }
    }
    LaunchedEffect(listState, dividerAt, messages.size, markEnabled, foreground) {
        // markEnabled is a key (not just a guard): the opening jump enables
        // marking after the first layout, and distinctUntilChanged would
        // otherwise swallow the initial viewport forever — a chat that fits
        // on screen with no scroll would never report anything viewed.
        snapshotFlow {
            val info = listState.layoutInfo
            val visible = info.visibleItemsInfo
            if (visible.isEmpty()) return@snapshotFlow Triple(true, 0UL, false)
            // reverseLayout: index 0 is the newest message at the bottom (above composer).
            val nearBottom = (visible.minOfOrNull { it.index } ?: 0) <= 1
            var max: ULong = 0UL
            for (v in visible) {
                val d = v.index
                if (dividerAt != null && d == dividerAt) continue
                val newestIndex = if (dividerAt != null && d > dividerAt) d - 1 else d
                val chronoIndex = messages.lastIndex - newestIndex
                val row = messages.getOrNull(chronoIndex) ?: continue
                if (row.convSeq > max) max = row.convSeq
            }
            Triple(nearBottom, max, listState.isScrollInProgress)
        }.distinctUntilChanged().collect { (nearBottom, maxSeq, scrolling) ->
            // Never re-pin mid-jump: only upward (away from bottom) updates
            // apply before marking starts.
            stickToBottom = if (nearBottom) markEnabled else false
            if (scrolling) userScrolled = true
            // Landing rows count only after the user scrolls, or when the
            // viewport holds the bottom (at-bottom reading / all fits).
            // Backgrounded layouts still report viewports, so marking is
            // gated on foreground: background arrivals stay unread and are
            // picked up when the effect restarts on return to foreground.
            if (foreground && markEnabled && (userScrolled || nearBottom) && maxSeq > 0UL) {
                latestVisibleRead.value(maxSeq)
            }
        }
    }
    var chatReady by remember(chat.id) { mutableStateOf(false) }
    LaunchedEffect(chat.id) {
        chatReady = false
        withFrameMillis { }
        chatReady = true
    }
    // Jump so a couple of seen rows, the marker, then the new rows are on
    // screen (heights vary, so placement verifies against the live layout).
    JumpToUnreadEffect(
        chatId = chat.id,
        messages = messages,
        entryMark = entryMark,
        entryUnreadSeq = entryUnreadSeq,
        openMaxSeq = openMaxSeq,
        listState = listState,
        onStickToBottom = { stickToBottom = it },
        onMarkEnabled = { markEnabled = it },
    )
    LaunchedEffect(lastKey, chat.id) {
        if (messages.isEmpty() || !stickToBottom) return@LaunchedEffect
        // Already pinned: reverseLayout inserts at index 0 without scrolling.
        // Calling scrollToItem again after insert is what makes the list "shake" once.
        val alreadyPinned =
            listState.firstVisibleItemIndex == 0 && listState.firstVisibleItemScrollOffset == 0
        if (alreadyPinned && chatReady) return@LaunchedEffect
        pinToBottomAnimated()
    }
    // Promote pending composer capture → active flight once the optimistic row exists.
    // Mobile only: desktop/web never flies, bubbles just appear.
    LaunchedEffect(lastKey) {
        if (!useFlyMorph) {
            pendingFly = null
            return@LaunchedEffect
        }
        val pending = pendingFly ?: return@LaunchedEffect
        val key = lastKey ?: return@LaunchedEffect
        if (!key.startsWith("local:")) return@LaunchedEffect
        val last = messages.lastOrNull() ?: return@LaunchedEffect
        if (messageListKey(last) != key || last.text != pending.text) return@LaunchedEffect
        pendingFly = null
        if (activeFlies.none { it.listKey == key }) {
            activeFlies.add(messageSendAnimationFor(last, key, pending.source, messages, outgoing))
        }
    }

    val requestSend: () -> Unit = requestSend@{
        if (!canSend) return@requestSend
        val text = draft.trim()
        if (text.isEmpty()) return@requestSend
        scope.launch {
            // Scroll to latest first so the new bubble lands above the composer, then send.
            if (!stickToBottom) {
                stickToBottom = true
                pinToBottomAnimated()
                withFrameMillis { }
            }
            val parent = overlayRoot
            val child = composerCoords
            // Capture the *visible* composer rect (includes IME padding on mobile).
            val from = if (parent != null && child != null) {
                boundsInParent(parent, child)
            } else {
                null
            }
            // Insert + clear draft. Mobile starts the morph overlay in the same turn so
            // text transfers continuously (no empty-composer frame before the fly begins).
            // Desktop/web has no morph — the bubble just appears via animateEnter.
            onSend()
            if (!useFlyMorph) {
                pendingFly = null
            } else if (from != null) {
                val last = messages.lastOrNull()
                val key = last?.let { messageListKey(it) }
                if (key != null && key.startsWith("local:") && last.text == text) {
                    pendingFly = null
                    if (activeFlies.none { it.listKey == key }) {
                        activeFlies.add(messageSendAnimationFor(last, key, from, messages, outgoing))
                    }
                } else {
                    pendingFly = PendingMessageSend(text = text, source = from)
                }
            } else {
                pendingFly = null
            }
        }
    }

    Box(
        Modifier
            .fillMaxSize()
            .onGloballyPositioned { coords ->
                overlayRoot = coords
                overlayWindowOrigin = coords.positionInWindow()
                rootHeightPx = coords.findRootCoordinates().size.height.toFloat()
            },
    ) {
        Scaffold(
            containerColor = wallpaper,
            modifier = Modifier.fillMaxSize(),
            topBar = {
                val pillColor = MaterialTheme.colorScheme.surface.copy(alpha = if (dark) 0.82f else 0.92f)
                val pillShape = RoundedCornerShape(22.dp)
                Row(
                    Modifier
                        .fillMaxWidth()
                        .statusBarsPadding()
                        .padding(horizontal = 10.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    if (showBack) {
                        Surface(
                            shape = CircleShape,
                            color = pillColor,
                            shadowElevation = 2.dp,
                            tonalElevation = 0.dp,
                        ) {
                            IconButton(onClick = onBack, modifier = Modifier.size(44.dp)) {
                                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                            }
                        }
                    }
                    Surface(
                        modifier = Modifier.weight(1f).clip(pillShape).combinedClickable(
                            onClick = onSettings,
                            onLongClick = { showMenagerie = true },
                        ),
                        shape = pillShape,
                        color = pillColor,
                        shadowElevation = 2.dp,
                        tonalElevation = 0.dp,
                    ) {
                        Row(
                            Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Avatar(chat.title, chat.isGroup, size = 34.dp)
                            Spacer(Modifier.width(10.dp))
                            Column(Modifier.weight(1f)) {
                                Text(
                                    chat.title,
                                    fontWeight = FontWeight.SemiBold,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                Text(
                                    if (chat.isGroup) "Group" else "End-to-end encrypted",
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                        }
                    }
                    Surface(
                        shape = pillShape,
                        color = pillColor,
                        shadowElevation = 2.dp,
                        tonalElevation = 0.dp,
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            if (!chat.isGroup) {
                                IconButton(onClick = onCall, modifier = Modifier.size(44.dp)) {
                                    Icon(Icons.Filled.Call, contentDescription = "Call")
                                }
                            }
                            Box {
                                IconButton(onClick = { onChatMenu(true) }, modifier = Modifier.size(44.dp)) {
                                    Icon(Icons.Filled.MoreVert, contentDescription = "More")
                                }
                                DropdownMenu(
                                    expanded = chatMenu,
                                    onDismissRequest = { onChatMenu(false) },
                                    containerColor = MaterialTheme.colorScheme.surface,
                                    tonalElevation = 0.dp,
                                    shadowElevation = 8.dp,
                                ) {
                                    DropdownMenuItem(
                                        text = { Text("Answer call") },
                                        onClick = {
                                            onChatMenu(false)
                                            onAnswer()
                                        },
                                    )
                                    DropdownMenuItem(
                                        text = { Text("Decline call") },
                                        onClick = {
                                            onChatMenu(false)
                                            onDecline()
                                        },
                                    )
                                    DropdownMenuItem(
                                        text = { Text("Hang up") },
                                        leadingIcon = { Icon(Icons.Filled.CallEnd, null) },
                                        onClick = {
                                            onChatMenu(false)
                                            onHangup()
                                        },
                                    )
                                    DropdownMenuItem(
                                        text = { Text("Chat settings") },
                                        onClick = {
                                            onChatMenu(false)
                                            onSettings()
                                        },
                                    )
                                }
                            }
                        }
                    }
                }
            },
            bottomBar = {
                // Floating dock detached from the viewport edges: the reply
                // card and the input pill hover over the chat with margins,
                // glass tints, and shadows instead of a full-bleed bar.
                Column(
                    Modifier
                        .fillMaxWidth()
                        .navigationBarsPadding()
                        .imePadding()
                        .padding(start = 8.dp, end = 8.dp, bottom = 8.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    if (replyTo != null) {
                        Surface(
                            shape = RoundedCornerShape(18.dp),
                            color = MaterialTheme.colorScheme.surface.copy(alpha = 0.95f),
                            shadowElevation = 8.dp,
                            tonalElevation = 0.dp,
                        ) {
                            Row(
                                Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = 12.dp, vertical = 6.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Box(
                                    Modifier
                                        .width(3.dp)
                                        .height(36.dp)
                                        .clip(RoundedCornerShape(2.dp))
                                        .background(
                                            peerAccentColor(
                                                replyTo.senderId.ifBlank {
                                                    replyTo.convId + replyTo.convSeq.toString()
                                                },
                                            ),
                                        ),
                                )
                                Spacer(Modifier.width(8.dp))
                                Column(Modifier.weight(1f)) {
                                    val replyAuthor = quoteAuthorName(replyTo, mine = false, contacts = contacts)
                                    if (replyAuthor.isNotBlank()) {
                                        Text(
                                            replyAuthor,
                                            style = MaterialTheme.typography.labelSmall,
                                            fontWeight = FontWeight.SemiBold,
                                            color = peerAccentColor(
                                                replyTo.senderId.ifBlank {
                                                    replyTo.convId + replyTo.convSeq.toString()
                                                },
                                            ),
                                            maxLines = 1,
                                        )
                                    }
                                    Text(
                                        shortQuoteText(
                                            if (replyTo.fileName.isNotEmpty()) {
                                                "📎 ${replyTo.fileName}"
                                            } else {
                                                replyTo.text
                                            },
                                        ),
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        maxLines = 1,
                                    )
                                }
                                IconButton(onClick = onClearReply) {
                                    Icon(
                                        Icons.Filled.Close,
                                        contentDescription = "Cancel reply",
                                    )
                                }
                            }
                        }
                    }
                    Row(
                        Modifier.fillMaxWidth(),
                        // Centers coincide single-line; the send cluster
                        // stays pinned to the bottom when the field grows.
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Surface(
                            modifier = Modifier
                                .weight(1f)
                                .onGloballyPositioned { composerCoords = it },
                            shape = RoundedCornerShape(24.dp),
                            color = MaterialTheme.colorScheme.surface.copy(alpha = 0.92f),
                            tonalElevation = 0.dp,
                            shadowElevation = 8.dp,
                        ) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                BasicTextField(
                                    value = draft,
                                    onValueChange = onDraft,
                                    modifier = Modifier
                                        .weight(1f)
                                        .heightIn(min = if (useFlyMorph) 36.dp else 40.dp)
                                        .padding(
                                            start = 16.dp,
                                            end = 4.dp,
                                            top = if (useFlyMorph) 8.dp else 10.dp,
                                            bottom = if (useFlyMorph) 8.dp else 10.dp,
                                        )
                                        .onPreviewKeyEvent { event ->
                                            if (event.type != KeyEventType.KeyDown) {
                                                return@onPreviewKeyEvent false
                                            }
                                            if (event.key != Key.Enter && event.key != Key.NumPadEnter) {
                                                return@onPreviewKeyEvent false
                                            }
                                            if (event.isShiftPressed) return@onPreviewKeyEvent false
                                            requestSend()
                                            true
                                        },
                                    textStyle = TextStyle(
                                        color = MaterialTheme.colorScheme.onSurface,
                                        fontSize = 16.sp,
                                        lineHeight = 20.sp,
                                    ),
                                    cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                                    maxLines = 5,
                                    // Soft keyboard keeps its Enter (newline) key — sending
                                    // is the round button's job. Hardware Enter
                                    // still sends via onPreviewKeyEvent above
                                    // (Shift+Enter falls through to newline).
                                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Default),
                                    keyboardActions = KeyboardActions.Default,
                                    decorationBox = { inner ->
                                        // No wrapping Box: it sizes to content and caps the
                                        // paragraph at ~119px, tripping wrap/scroll (and a
                                        // misplaced cursor) once text passes it. inner()
                                        // fills the field on its own.
                                        if (draft.isEmpty()) {
                                            Box {
                                                Text(
                                                    "Message",
                                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                                    fontSize = 16.sp,
                                                )
                                                inner()
                                            }
                                        } else {
                                            inner()
                                        }
                                    },
                                )
                                IconButton(onClick = onAttach) {
                                    Icon(
                                        Icons.Filled.AttachFile,
                                        contentDescription = "Attach",
                                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                        }
                        Spacer(Modifier.width(6.dp))
                        val limitFraction = (draftBytes / TEXT_MAX_BYTES.toFloat()).coerceIn(0f, 1f)
                        val ringColor = if (overLimit) {
                            MaterialTheme.colorScheme.error
                        } else {
                            MaterialTheme.colorScheme.primary
                        }
                        Box(
                            modifier = Modifier
                                .scale(sendScale)
                                .size(52.dp)
                                .align(Alignment.Bottom),
                            contentAlignment = Alignment.Center,
                        ) {
                            Canvas(Modifier.fillMaxSize()) {
                                if (draft.isNotEmpty()) {
                                    val pad = 2.dp.toPx()
                                    drawArc(
                                        color = ringColor,
                                        startAngle = -90f,
                                        sweepAngle = 360f * limitFraction,
                                        useCenter = false,
                                        topLeft = Offset(pad, pad),
                                        size = Size(size.width - pad * 2, size.height - pad * 2),
                                        style = Stroke(width = 2.5.dp.toPx(), cap = StrokeCap.Round),
                                    )
                                }
                            }
                            FilledIconButton(
                                onClick = requestSend,
                                enabled = canSend,
                                modifier = Modifier.size(46.dp),
                                shape = CircleShape,
                            ) {
                                Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Send")
                            }
                        }
                    }
                }
            },
        ) { padding ->
            Box(
                Modifier
                    .fillMaxSize()
                    .padding(bottom = padding.calculateBottomPadding()),
            ) {
                ChatWallpaper(modifier = Modifier.fillMaxSize())
                // Bubbles only: reaction pills render under their target, transient
                // remove markers never render. Full `messages` (incl. reactions)
                // is kept for pills/quotes via DatedMessageItem.
                val bubbles = remember(messages) { messages.filter(::isBubbleRow) }
                // reverseLayout stacks short threads on the composer (empty space above).
                val newestFirst = remember(bubbles) { bubbles.asReversed() }
                val totalCount = newestFirst.size + if (dividerAt != null) 1 else 0
                LazyColumn(
                    state = listState,
                    reverseLayout = true,
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(
                        start = 10.dp,
                        end = 10.dp,
                        top = padding.calculateTopPadding() + 10.dp,
                        bottom = 10.dp,
                    ),
                ) {
                    items(
                        count = totalCount,
                        key = { display ->
                            if (display == dividerAt) {
                                "unread-marker"
                            } else {
                                val newestIndex = if (dividerAt != null && display > dividerAt) display - 1 else display
                                messageListKey(newestFirst[newestIndex])
                            }
                        },
                    ) { display ->
                        if (display == dividerAt) {
                            UnreadMarker()
                        } else {
                            val newestIndex = if (dividerAt != null && display > dividerAt) display - 1 else display
                            val row = newestFirst[newestIndex]
                            // Chronological neighbors for clustering (list is newest-first).
                            val chronoIndex = bubbles.lastIndex - newestIndex
                            DatedMessageItem(
                                row = row,
                                chronoIndex = chronoIndex,
                                messages = bubbles,
                                allMessages = messages,
                                outgoing = outgoing,
                                outgoingStatus = outgoingStatus,
                                floatingDate = floatingDate,
                                seedDone = seedDone,
                                knownKeys = knownKeys,
                                useFlyMorph = useFlyMorph,
                                flyingKeys = flyingKeys,
                                overlayRoot = overlayRoot,
                                flyTargets = flyTargets,
                                onReact = onReact,
                                onDelete = onDelete,
                                onReply = onReply,
                                onPillClick = onPillClick,
                                onPillLongClick = onPillLongClick,
                                contacts = contacts,
                                onCopy = onCopy,
                                onQuickReact = onQuickReact,
                                onSave = onSave,
                                onAdmitJoin = onAdmitJoin,
                                onAcceptInvite = onAcceptInvite,
                                acceptedInvites = acceptedInvites,
                                admittedJoins = admittedJoins,
                                showSenderNames = showSenderNames,
                            )
                        }
                    }
                }
                MessageListTopFade(baseColor = wallpaper, modifier = Modifier.align(Alignment.TopCenter))
                FloatingChatDate(
                    dateText = floatingDate.activeLabel,
                    visible = floatingDate.floatingEffectiveVisible,
                    offsetYPx = floatingDate.floatingPush,
                    modifier = Modifier.align(Alignment.TopCenter).padding(top = padding.calculateTopPadding() + 8.dp),
                    onPositioned = { top, h -> floatingDate.onFloatingPositioned(top, h) },
                )
                val firstVisible = listState.firstVisibleItemIndex
                val belowNew = remember(messages, readMark, dividerAt, firstVisible) {
                    belowUnreadCount(messages, chat.id, readMark, dividerAt, firstVisible)
                }
                if (belowNew > 0 && !stickToBottom) {
                    JumpLatestButton(
                        count = belowNew,
                        modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 12.dp),
                        onClick = {
                            scope.launch {
                                stickToBottom = true
                                autoScrolling = true
                                try {
                                    listState.animateScrollToItem(0)
                                } finally {
                                    autoScrolling = false
                                }
                            }
                        },
                    )
                }
            }
        }

        // Mobile only overlay: composer → bubble morph (not a list-item slide-in).
        // Desktop intentionally has no overlay — bubbles appear in place.
        if (useFlyMorph) {
            for (fly in activeFlies.toList()) {
                key(fly.listKey) {
                    MessageSendFlyOverlay(
                        animation = fly,
                        liveTarget = flyTargets[fly.listKey],
                        overlayWindowOrigin = overlayWindowOrigin,
                        rootHeightPx = rootHeightPx,
                        dark = dark,
                        onFinished = {
                            activeFlies.removeAll { it.listKey == fly.listKey }
                            flyTargets.remove(fly.listKey)
                        },
                    )
                }
            }
        }
        if (showMenagerie) {
            MenagerieEasterEggDialog(onDismiss = { showMenagerie = false })
        }
    }
}

/**
 * Single place that hosts a visible chat thread.
 *
 * Owns its file-picker launcher and mic gate per [target] so attach/call always
 * hit the thread on screen (never a stale `selected`). Lets the split
 * [Crossfade] and the mobile push [AnimatedContent] share identical behavior.
 */
@Composable
private fun ActiveChatThread(
    c: NemoClient?,
    target: ChatTarget,
    messages: MutableList<DisplayRow>,
    outgoing: MutableMap<String, Boolean>,
    outgoingStatus: MutableMap<String, OutgoingStatus>,
    draft: String,
    onDraft: (String) -> Unit,
    showBack: Boolean,
    onBack: () -> Unit,
    onSettings: () -> Unit,
    chatMenu: Boolean,
    onChatMenu: (Boolean) -> Unit,
    scope: CoroutineScope,
    snackbar: SnackbarHostState,
    runIo: (suspend () -> Unit) -> Unit,
    onMarkOutgoing: (DisplayRow) -> Unit,
    saveFile: (String, ByteArray) -> Unit,
    lastRead: MutableMap<String, ULong>,
    vaultDir: File,
    readReceiptsEnabled: Boolean,
    isForeground: Boolean,
    onAcceptFallback: ((String) -> Unit)? = null,
    acceptedInvites: MutableSet<String> = mutableSetOf(),
    admittedJoins: MutableSet<String> = mutableSetOf(),
    contacts: Map<String, String> = emptyMap(),
) {
    val pickFile = rememberPickFile { path ->
        attachFilePath(
            c,
            target,
            path,
            messages,
            outgoing,
            outgoingStatus,
            snackbar,
            scope,
        )
    }
    var micAction by remember(target.id) { mutableStateOf<(() -> Unit)?>(null) }
    val requestMic = rememberEnsureMic { micAction?.invoke() }
    var replyTo by remember(target.id) { mutableStateOf<DisplayRow?>(null) }
    var deleteTarget by remember(target.id) { mutableStateOf<DisplayRow?>(null) }
    var reactTarget by remember(target.id) { mutableStateOf<DisplayRow?>(null) }
    var authorsTarget by remember(target.id) { mutableStateOf<DisplayRow?>(null) }
    // Peer read receipts: highest viewed seq this visit. Flushed (not streamed)
    // on leave, on background, and as soon as the newest message is visible —
    // never from background fetch. 1:1 only, gated by the setting.
    var maxViewedRead by remember(target.id) { mutableStateOf(0UL) }
    var lastSentRead by remember(target.id) { mutableStateOf(0UL) }
    fun flushReadMark() {
        if (!readReceiptsEnabled || target.isGroup) return
        val upto = maxViewedRead
        if (upto == 0UL || upto <= lastSentRead) return
        lastSentRead = upto
        val cc = c ?: return
        val id = target.id
        scope.launch(Dispatchers.IO) { runCatching { cc.markRead(id, upto) } }
    }
    DisposableEffect(target.id) {
        onDispose { flushReadMark() }
    }
    LaunchedEffect(isForeground) {
        if (!isForeground) flushReadMark()
    }
    ChatThread(
        chat = target,
        messages = messages.filter { it.convId == target.id },
        entryMark = remember(target.id) { lastRead[target.id] ?: 0UL },
        readMark = lastRead[target.id] ?: 0UL,
        foreground = isForeground,
        onVisibleRead = { seq ->
            markVisibleRead(lastRead, vaultDir, scope, target.id, seq)
            // Viewport-tracked peer receipts: accumulate the max, flush when
            // the newest message is on screen (live reading). Leave and
            // background flush the rest (see effects above).
            if (seq > maxViewedRead) {
                maxViewedRead = seq
                val latest = messages.filter { it.convId == target.id }.maxOfOrNull { it.convSeq } ?: 0UL
                if (latest > 0UL && seq >= latest) flushReadMark()
            }
        },
        outgoing = outgoing,
        outgoingStatus = outgoingStatus,
        draft = draft,
        onDraft = onDraft,
        showBack = showBack,
        onBack = onBack,
        onSettings = onSettings,
        chatMenu = chatMenu,
        onChatMenu = onChatMenu,
        onSend = {
            val rt = replyTo?.convSeq ?: 0UL
            sendChat(
                c,
                target,
                draft,
                messages,
                outgoing,
                outgoingStatus,
                scope,
                snackbar,
                clear = {
                    onDraft("")
                    replyTo = null
                },
                replyTo = rt,
            )
        },
        onAttach = { pickFile() },
        onCall = {
            micAction = {
                runIo {
                    val row = withContext(Dispatchers.IO) {
                        c?.startCall(target.id)
                    } ?: return@runIo
                    messages.add(row)
                    onMarkOutgoing(row)
                    c?.let { startCallAudio(it) }
                }
            }
            requestMic()
        },
        onAnswer = {
            micAction = {
                runIo {
                    val id = messages.lastOrNull { it.kind == "call_invite" }?.text
                        ?: return@runIo
                    val row = withContext(Dispatchers.IO) {
                        c?.answerCall(id)
                    } ?: return@runIo
                    messages.add(row)
                    c?.let { startCallAudio(it) }
                }
            }
            requestMic()
        },
        onHangup = {
            runIo {
                stopCallAudio()
                val row = withContext(Dispatchers.IO) { c?.endCall() }
                    ?: return@runIo
                messages.add(row)
            }
        },
        onDecline = {
            runIo {
                val id = messages.lastOrNull { it.kind == "call_invite" }?.text
                    ?: return@runIo
                stopCallAudio()
                val row = withContext(Dispatchers.IO) {
                    c?.rejectCall(id)
                } ?: return@runIo
                messages.add(row)
            }
        },
        onReact = { row ->
            reactTarget = row
        },
        onReply = { row ->
            replyTo = row
        },
        onPillClick = { row, emoji ->
            runIo {
                val r = withContext(Dispatchers.IO) {
                    c?.react(target.id, row.convSeq, emoji)
                } ?: return@runIo
                applyIncoming(messages, listOf(r), outgoing, outgoingStatus)
                if (r.outgoing) onMarkOutgoing(r)
            }
        },
        onPillLongClick = { row ->
            authorsTarget = row
        },
        onCopy = { row ->
            copyToClipboard(row.text.ifBlank { row.fileName })
            scope.launch {
                snackbar.showSnackbar("Copied")
            }
        },
        onQuickReact = { row, emoji ->
            runIo {
                val r = withContext(Dispatchers.IO) {
                    c?.react(target.id, row.convSeq, emoji)
                } ?: return@runIo
                applyIncoming(messages, listOf(r), outgoing, outgoingStatus)
                if (r.outgoing) onMarkOutgoing(r)
            }
        },
        onDelete = { row ->
            deleteTarget = row
        },
        onSave = { row ->
            if (canSaveAttachment(row)) {
                saveFile(row.fileName, row.fileBytes)
            } else {
                scope.launch {
                    snackbar.showSnackbar("File unavailable")
                }
            }
        },
        replyTo = replyTo,
        onClearReply = { replyTo = null },
        contacts = contacts,
        onAdmitJoin = { row ->
            runIo {
                withContext(Dispatchers.IO) {
                    c?.admitJoin(row.text.trim())
                }
                admittedJoins.add(messageListKey(row))
                snackbar.showSnackbar("Admitted member")
            }
        },
        onAcceptInvite = { row ->
            runIo {
                val uri = row.text.trim()
                val sender = row.convId
                val j = withContext(Dispatchers.IO) {
                    c?.acceptGroupInvite(uri).orEmpty()
                }
                copyToClipboard(j)
                acceptedInvites.add(messageListKey(row))
                val sent = runCatching {
                    withContext(Dispatchers.IO) {
                        c?.sendText(sender, j)
                    }
                }.getOrNull()
                if (sent != null) {
                    messages.add(sent)
                    onMarkOutgoing(sent)
                    snackbar.showSnackbar("Join request sent — they tap Admit")
                } else {
                    // Invite came from outside a 1:1 (group thread,
                    // clipboard): fall back to manual send.
                    onAcceptFallback?.invoke(j)
                }
            }
        },
        acceptedInvites = acceptedInvites,
        admittedJoins = admittedJoins,
        showSenderNames = target.isGroup,
    )
    if (deleteTarget != null) {
        val row = deleteTarget!!
        AlertDialog(
            onDismissRequest = { deleteTarget = null },
            title = { Text("Delete message?") },
            text = { Text("Delete for everyone or only for you? Everyone removes it on all devices; only-you keeps it for others.") },
            confirmButton = {
                TextButton(onClick = {
                    val t = row
                    deleteTarget = null
                    replyTo?.let { if (it.convSeq == t.convSeq && it.convId == t.convId) replyTo = null }
                    runIo {
                        val r = withContext(Dispatchers.IO) {
                            c?.deleteMessage(target.id, t.convSeq)
                        } ?: return@runIo
                        applyIncoming(messages, listOf(r), outgoing, outgoingStatus)
                    }
                }) { Text("Everyone") }
            },
            dismissButton = {
                Row {
                    TextButton(onClick = { deleteTarget = null }) { Text("Cancel") }
                    TextButton(onClick = {
                        val t = row
                        deleteTarget = null
                        replyTo?.let { if (it.convSeq == t.convSeq && it.convId == t.convId) replyTo = null }
                        runIo {
                            val r = withContext(Dispatchers.IO) {
                                c?.deleteMessageForMe(target.id, t.convSeq)
                            } ?: return@runIo
                            applyIncoming(messages, listOf(r), outgoing, outgoingStatus)
                        }
                    }) { Text("Only me") }
                }
            },
        )
    }
    if (reactTarget != null) {
        val row = reactTarget!!
        AlertDialog(
            onDismissRequest = { reactTarget = null },
            title = { Text("React") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("Tap to toggle. You can add multiple different emojis.")
                    @OptIn(ExperimentalLayoutApi::class)
                    FlowRow(
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        for (emoji in QUICK_REACTIONS) {
                            val mine = reactionsFor(messages, row.convId, row.convSeq, contacts)
                                .any { it.emoji == emoji && it.mine }
                            FilterChip(
                                selected = mine,
                                onClick = {
                                    reactTarget = null
                                    runIo {
                                        val r = withContext(Dispatchers.IO) {
                                            c?.react(target.id, row.convSeq, emoji)
                                        } ?: return@runIo
                                        applyIncoming(messages, listOf(r), outgoing, outgoingStatus)
                                        if (r.outgoing) onMarkOutgoing(r)
                                    }
                                },
                                label = { Text(emoji) },
                            )
                        }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { reactTarget = null }) { Text("Close") }
            },
        )
    }
    if (authorsTarget != null) {
        val row = authorsTarget!!
        val pills = reactionsFor(messages, row.convId, row.convSeq, contacts)
        AlertDialog(
            onDismissRequest = { authorsTarget = null },
            title = { Text("Reactions") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    if (pills.isEmpty()) {
                        Text("No reactions yet.")
                    } else {
                        for (pill in pills) {
                            Text("${pill.emoji} × ${pill.count}", fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)
                            for (a in pill.authors) {
                                Text("· ${a.displayName}")
                            }
                        }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { authorsTarget = null }) { Text("Close") }
            },
        )
    }
}

private fun messageSendAnimationFor(
    row: DisplayRow,
    listKey: String,
    source: Rect,
    messages: List<DisplayRow>,
    outgoing: Map<String, Boolean>,
): MessageSendAnimation {
    val idx = messages.indexOfFirst { messageListKey(it) == listKey }.coerceAtLeast(0)
    val mine = true
    val prevMine = messages.getOrNull(idx - 1)?.let { prev ->
        prev.outgoing || outgoing[outgoingMapKey(prev)] == true
    }
    val nextMine = messages.getOrNull(idx + 1)?.let { next ->
        next.outgoing || outgoing[outgoingMapKey(next)] == true
    }
    return MessageSendAnimation(
        listKey = listKey,
        text = row.text,
        timeLabel = formatTime(row.sentAt),
        source = source,
        clusteredAbove = prevMine == mine,
        clusteredBelow = nextMine == mine,
    )
}

@Composable
private fun ChatWallpaper(modifier: Modifier = Modifier) {
    val palette = LocalNemoPalette.current
    val base = palette.chat
    val dot = palette.patternDot
    Canvas(modifier.background(base)) {
        val step = 28.dp.toPx()
        var y = step * 0.5f
        var row = 0
        while (y < size.height + step) {
            var x = if (row % 2 == 0) step * 0.35f else step * 0.85f
            while (x < size.width + step) {
                drawCircle(color = dot, radius = 1.6.dp.toPx(), center = Offset(x, y))
                x += step
            }
            y += step * 0.72f
            row++
        }
    }
}

@Composable
internal fun MessageBubble(
    row: DisplayRow,
    mine: Boolean,
    status: OutgoingStatus?,
    clusteredAbove: Boolean,
    clusteredBelow: Boolean,
    animateEnter: Boolean,
    onReact: () -> Unit,
    onDelete: () -> Unit,
    onSave: () -> Unit,
    modifier: Modifier = Modifier,
    conceal: Boolean = false,
    onBubbleCoords: ((LayoutCoordinates) -> Unit)? = null,
    onAdmitJoin: ((DisplayRow) -> Unit)? = null,
    onAcceptInvite: ((DisplayRow) -> Unit)? = null,
    inviteAccepted: Boolean = false,
    joinAdmitted: Boolean = false,
    showSenderNames: Boolean = false,
    onReply: () -> Unit = {},
    pills: List<ReactionPill> = emptyList(),
    quote: DisplayRow? = null,
    onPillClick: (String) -> Unit = {},
    onPillLongClick: () -> Unit = {},
    onCopy: () -> Unit = {},
    onQuickReact: (String) -> Unit = {},
    contacts: Map<String, String> = emptyMap(),
) {
    var menu by remember { mutableStateOf(false) }
    val dark = nemoDarkTheme()
    // Appear animation: desktop has no composer morph — new bubbles fade/scale/rise
    // in place (250 ms cubic-bezier(.4,0,.2,1)). Mobile keeps its morph overlay for
    // outgoing sends; incoming bubbles on both platforms use this subtle appear.
    val mobileFeel = messageSendFlyMobileFeel()
    val appearEasing = if (mobileFeel) EaseOutCubic else CubicBezierEasing(0.4f, 0f, 0.2f, 1f)
    val appearMs = if (mobileFeel) 180 else 250
    val enter = remember { Animatable(if (animateEnter) 0f else 1f) }
    LaunchedEffect(Unit) {
        if (animateEnter) {
            enter.animateTo(
                targetValue = 1f,
                animationSpec = tween(durationMillis = appearMs, easing = appearEasing),
            )
        }
    }
    // Window-Y of this bubble + root height → sample one continuous screen gradient.
    var windowY by remember { mutableFloatStateOf(0f) }
    var rootHeight by remember { mutableFloatStateOf(1f) }
    // Bubble geometry for the anchored context menu (flip above/below on edges).
    var bubbleHeightPx by remember { mutableFloatStateOf(0f) }
    val palette = LocalNemoPalette.current
    val outgoingBrush = if (mine) {
        outgoingScreenBrush(palette.outgoingGradient, windowY, rootHeight)
    } else {
        null
    }
    val incomingBg = palette.incoming
    val corner = 14.dp
    val tight = 5.dp
    val tail = 4.dp
    val shape = if (mine) {
        RoundedCornerShape(
            topStart = corner,
            topEnd = if (clusteredAbove) tight else corner,
            bottomStart = corner,
            bottomEnd = if (clusteredBelow) tight else tail,
        )
    } else {
        RoundedCornerShape(
            topStart = if (clusteredAbove) tight else corner,
            topEnd = corner,
            bottomStart = if (clusteredBelow) tight else tail,
            bottomEnd = corner,
        )
    }
    val shadow = palette.bubbleShadow
    // Mono bubbles are inverse (black in Mono Light, white in Mono Dark),
    // so own text must contrast the bubble there. Blue branches unchanged.
    val metaColor = when {
        mine && palette.mono && dark -> Color(0xFF000000).copy(alpha = 0.65f)
        mine && palette.mono -> Color(0xFFFFFFFF).copy(alpha = 0.75f)
        mine && dark -> Color(0xFFB8D4E8).copy(alpha = 0.9f)
        else -> MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.85f)
    }
    val bodyColor = when {
        mine && palette.mono && dark -> Color(0xFF000000)
        mine && palette.mono -> Color(0xFFFFFFFF)
        mine && dark -> Color(0xFFE8F4FF)
        else -> MaterialTheme.colorScheme.onSurface
    }
    val t = enter.value
    val density = LocalDensity.current
    val risePx = with(density) { (if (mobileFeel) 4.dp else 8.dp).toPx() } * (1f - t)
    val appearScale = if (mobileFeel) {
        1f
    } else {
        androidx.compose.ui.util.lerp(0.9f, 1f, appearEasing.transform(t))
    }
    Row(
        modifier
            .fillMaxWidth()
            .graphicsLayer {
                alpha = if (conceal) 0f else t
                scaleX = appearScale
                scaleY = appearScale
                translationY = risePx
                transformOrigin = androidx.compose.ui.graphics.TransformOrigin(
                    if (mine) 1f else 0f,
                    1f,
                )
            },
        horizontalArrangement = if (mine) Arrangement.End else Arrangement.Start,
    ) {
        Box {
            val bigEmoji = isBigEmojiMessage(row)
            Column(
                Modifier
                    .widthIn(max = 320.dp)
                    .onGloballyPositioned { coords ->
                        onBubbleCoords?.invoke(coords)
                        windowY = coords.positionInWindow().y
                        bubbleHeightPx = coords.size.height.toFloat()
                        rootHeight = coords.findRootCoordinates().size.height.toFloat()
                    },
                horizontalAlignment = if (mine) Alignment.End else Alignment.Start,
            ) {
                if (bigEmoji) {
                    BigEmojiMessage(
                        row = row,
                        mine = mine,
                        status = if (mine) status else null,
                        onOpenMenu = { menu = true },
                    )
                } else {
                    Column(
                        Modifier
                            .shadow(2.dp, shape, ambientColor = shadow, spotColor = shadow)
                            .clip(shape)
                            .then(
                                if (outgoingBrush != null) {
                                    Modifier.background(outgoingBrush)
                                } else {
                                    Modifier.background(incomingBg)
                                },
                            )
                            .combinedClickable(
                                interactionSource = remember { MutableInteractionSource() },
                                indication = LocalIndication.current,
                                enabled = !conceal,
                                onClick = { menu = true },
                                onLongClick = { menu = true },
                            )
                            .padding(horizontal = 12.dp, vertical = 6.dp),
                    ) {
                        val senderLabel = if (showSenderNames && !mine) {
                            row.senderName.ifBlank {
                                shortId(row.senderId).ifBlank { "" }
                            }
                        } else {
                            ""
                        }
                        if (senderLabel.isNotEmpty()) {
                            Text(
                                text = senderLabel,
                                style = MaterialTheme.typography.labelSmall,
                                fontWeight = FontWeight.SemiBold,
                                color = peerAccentColor(row.senderId.ifBlank { row.convId }),
                                maxLines = 1,
                            )
                        }
                        if (isGroupInviteUri(row.text) && onAcceptInvite != null) {
                            GroupInviteCard(
                                mine = mine,
                                timeLabel = formatTime(row.sentAt),
                                bodyColor = bodyColor,
                                metaColor = metaColor,
                                hasStatus = mine && status != null,
                                status = {
                                    if (mine && status != null) {
                                        DeliveryTicks(status = status, tint = metaColor)
                                    }
                                },
                                onAccept = if (!mine && !inviteAccepted) {
                                    { onAcceptInvite(row) }
                                } else {
                                    null
                                },
                                accepted = inviteAccepted,
                            )
                        } else if (isJoinRequestUri(row.text) && onAdmitJoin != null) {
                            JoinRequestCard(
                                mine = mine,
                                timeLabel = formatTime(row.sentAt),
                                bodyColor = bodyColor,
                                metaColor = metaColor,
                                hasStatus = mine && status != null,
                                status = {
                                    if (mine && status != null) {
                                        DeliveryTicks(status = status, tint = metaColor)
                                    }
                                },
                                onAdmit = if (!mine && !joinAdmitted) {
                                    { onAdmitJoin(row) }
                                } else {
                                    null
                                },
                                admitted = joinAdmitted,
                            )
                        } else {
                            // Reply header: accent bar + colored name + preview.
                            // Hidden entirely when the original was hard-deleted.
                            if (quote != null) {
                                ReplyQuoteHeader(
                                    quote = quote,
                                    mine = mine,
                                    bodyColor = bodyColor,
                                    contacts = contacts,
                                )
                                Spacer(Modifier.height(4.dp))
                            }
                            BubbleContent(
                                text = bubbleText(row),
                                timeLabel = formatTime(row.sentAt),
                                bodyColor = bodyColor,
                                metaColor = metaColor,
                                hasStatus = mine && status != null,
                                status = {
                                    if (mine && status != null) {
                                        DeliveryTicks(status = status, tint = metaColor)
                                    }
                                },
                            )
                        }
                    }
                }
                // Reaction pills float below the bubble, aligned to its edge.
                if (pills.isNotEmpty()) {
                    Spacer(Modifier.height(2.dp))
                    ReactionPills(
                        pills = pills,
                        onPillClick = onPillClick,
                        onPillLongClick = onPillLongClick,
                    )
                }
            }
            if (menu) {
                // Anchored near the bubble: flip above/below + align to the
                // bubble edge so it never centers on screen.
                val canCopy = row.text.isNotBlank() || row.fileName.isNotBlank()
                val canSave = canSaveAttachment(row)
                val actionCount = 2 + (if (canCopy) 1 else 0) + (if (canSave) 1 else 0)
                val menuHeightPx = with(density) { (72.dp + 8.dp).toPx() } +
                    actionCount * with(density) { 50.dp.toPx() } +
                    with(density) { 12.dp.toPx() }
                val gapPx = with(density) { 8.dp.toPx() }
                val spaceAbove = windowY
                val spaceBelow = (rootHeight - windowY - bubbleHeightPx).coerceAtLeast(0f)
                val showAbove = spaceAbove >= menuHeightPx || spaceAbove > spaceBelow
                val yOffset = if (showAbove) {
                    -(menuHeightPx + gapPx).toInt()
                } else {
                    (bubbleHeightPx + gapPx).toInt()
                }
                Popup(
                    alignment = if (mine) Alignment.TopEnd else Alignment.TopStart,
                    offset = IntOffset(0, yOffset),
                    onDismissRequest = { menu = false },
                    properties = PopupProperties(
                        focusable = true,
                        dismissOnBackPress = true,
                        dismissOnClickOutside = true,
                    ),
                ) {
                    var shown by remember { mutableStateOf(false) }
                    LaunchedEffect(Unit) { shown = true }
                    val pop by animateFloatAsState(
                        targetValue = if (shown) 1f else 0.85f,
                        animationSpec = spring(
                            dampingRatio = Spring.DampingRatioMediumBouncy,
                            stiffness = Spring.StiffnessMediumLow,
                        ),
                        label = "menuPop",
                    )
                    Box(
                        Modifier.graphicsLayer {
                            scaleX = pop
                            scaleY = pop
                            transformOrigin = androidx.compose.ui.graphics.TransformOrigin(
                                if (mine) 1f else 0f,
                                if (showAbove) 1f else 0f,
                            )
                        },
                    ) {
                        FluidMessageMenuContent(
                            target = row,
                            pills = pills,
                            canSave = canSave,
                            canCopy = canCopy,
                            onQuickReact = { emoji ->
                                menu = false
                                onQuickReact(emoji)
                            },
                            onExpandReactions = {
                                menu = false
                                onReact()
                            },
                            onReply = {
                                menu = false
                                onReply()
                            },
                            onCopy = {
                                menu = false
                                onCopy()
                            },
                            onSave = {
                                menu = false
                                onSave()
                            },
                            onDelete = {
                                menu = false
                                onDelete()
                            },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun BigEmojiMessage(row: DisplayRow, mine: Boolean, status: OutgoingStatus?, onOpenMenu: () -> Unit) {
    Column(
        horizontalAlignment = if (mine) Alignment.End else Alignment.Start,
        modifier = Modifier.combinedClickable(
            interactionSource = remember { MutableInteractionSource() },
            indication = LocalIndication.current,
            onClick = onOpenMenu,
            onLongClick = onOpenMenu,
        ),
    ) {
        Text(
            text = row.text,
            fontSize = 76.sp,
            lineHeight = 84.sp,
        )
        Spacer(Modifier.height(4.dp))
        Surface(
            shape = RoundedCornerShape(10.dp),
            color = Color.Black.copy(alpha = 0.38f),
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                Text(
                    text = formatTime(row.sentAt),
                    style = MaterialTheme.typography.labelSmall,
                    color = Color.White.copy(alpha = 0.9f),
                )
                if (mine && status != null) {
                    DeliveryTicks(status = status, tint = Color.White.copy(alpha = 0.85f))
                }
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ReactionPills(pills: List<ReactionPill>, onPillClick: (String) -> Unit, onPillLongClick: () -> Unit) {
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        for (pill in pills) {
            Surface(
                shape = RoundedCornerShape(12.dp),
                color = if (pill.mine) {
                    MaterialTheme.colorScheme.primaryContainer
                } else {
                    MaterialTheme.colorScheme.surfaceVariant
                },
                tonalElevation = 0.dp,
                shadowElevation = 0.dp,
                modifier = Modifier.combinedClickable(
                    onClick = { onPillClick(pill.emoji) },
                    onLongClick = onPillLongClick,
                ),
            ) {
                Text(
                    text = if (pill.count > 1) "${pill.emoji} ${pill.count}" else pill.emoji,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp),
                )
            }
        }
    }
}

@Composable
private fun DeliveryTicks(status: OutgoingStatus, tint: Color) {
    val (icon, description) = when (status) {
        OutgoingStatus.Pending -> Icons.Filled.AccessTime to "Sending"
        OutgoingStatus.Sent -> Icons.Filled.Done to "Sent"
        OutgoingStatus.Delivered -> Icons.Filled.DoneAll to "Delivered"
        OutgoingStatus.Read -> Icons.Filled.DoneAll to "Read"
        OutgoingStatus.Failed -> Icons.Filled.ErrorOutline to "Failed"
    }
    val color = when (status) {
        OutgoingStatus.Failed -> MaterialTheme.colorScheme.error
        // Mono bubbles are inverse, so the brand accent is invisible on them —
        // ticks follow the message text instead. Blue keeps blue.
        // Read is full emphasis; delivered is the same hue at reduced alpha
        // so the pair adapts to every theme without hardcoded colors.
        OutgoingStatus.Read ->
            if (LocalNemoPalette.current.mono) {
                LocalNemoPalette.current.mineBody
            } else {
                MaterialTheme.colorScheme.primary
            }
        OutgoingStatus.Delivered ->
            if (LocalNemoPalette.current.mono) {
                LocalNemoPalette.current.mineBody.copy(alpha = 0.6f)
            } else {
                MaterialTheme.colorScheme.primary.copy(alpha = 0.6f)
            }
        else -> tint
    }
    Icon(
        icon,
        contentDescription = description,
        modifier = Modifier.size(14.dp),
        tint = color,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsScreen(
    fingerprint: String,
    identityHex: String,
    homeUrl: String,
    onHomeUrl: (String) -> Unit,
    shareUri: String,
    joinUri: String,
    joinPaste: String,
    onJoinPaste: (String) -> Unit,
    inviteUri: String,
    inviteSendTo: String,
    onInviteSendTo: (String) -> Unit,
    roster: Map<String, String>,
    onSendInvite: () -> Unit,
    memberCred: String,
    disappearSecs: String,
    onDisappearSecs: (String) -> Unit,
    contactNickname: String,
    onContactNickname: (String) -> Unit,
    selected: ChatTarget?,
    busy: Boolean,
    onBack: () -> Unit,
    onRegister: () -> Unit,
    onShare: () -> Unit,
    onAdmit: () -> Unit,
    onInviteGroup: () -> Unit,
    onSaveNickname: () -> Unit,
    onDisappear: () -> Unit,
    privacyMode: String,
    onPrivacyMode: (String) -> Unit,
    readReceiptsEnabled: Boolean,
    onReadReceipts: (Boolean) -> Unit,
    notifyMode: NemoNotifyMode,
    onNotifyMode: (NemoNotifyMode) -> Unit,
    notificationsAllowed: Boolean,
    onOpenSystemSettings: () -> Unit,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
            )
        },
    ) { padding ->
        LazyColumn(
            Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            contentPadding = PaddingValues(bottom = 32.dp, top = 8.dp),
        ) {
            if (selected != null && !selected.isGroup) {
                item {
                    Text("Contact name", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                    Text(
                        "Local nickname only. Not shared with the other person.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    OutlinedTextField(
                        value = contactNickname,
                        onValueChange = onContactNickname,
                        label = { Text("Name") },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                    )
                    Button(
                        enabled = !busy,
                        onClick = onSaveNickname,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text("Save name")
                    }
                }
            }
            item {
                Text("Appearance", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                Text(
                    "Light and Dark use Nemo's blue chat theme. Mono Light and Mono Dark are black and white. System follows the OS.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                val themeMode = LocalThemeMode.current
                val onThemeMode = LocalOnThemeModeChange.current
                Row(
                    Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState())
                        .padding(top = 4.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    NemoThemeMode.entries.forEach { mode ->
                        FilterChip(
                            selected = themeMode == mode,
                            onClick = { onThemeMode(mode) },
                            label = { Text(mode.label) },
                        )
                    }
                }
            }
            item {
                Text("Identity", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                Text("Fingerprint", style = MaterialTheme.typography.labelSmall)
                SelectionContainer {
                    Text(fingerprint, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                }
                Text("identity_id", style = MaterialTheme.typography.labelSmall)
                SelectionContainer {
                    Text(identityHex, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                }
            }
            item {
                Text("Home server", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                OutlinedTextField(
                    value = homeUrl,
                    onValueChange = onHomeUrl,
                    label = { Text("Home URL") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                )
                Button(enabled = !busy, onClick = onRegister, modifier = Modifier.fillMaxWidth()) {
                    Text("Connect")
                }
            }
            item {
                Text("Privacy", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                Text(
                    "Envelope bytes never change. High sends dummy envelopes to a contact. " +
                        "Maximum keeps a ~2s slot and turns calls off. " +
                        "Tor is used for client→home when the home is not on loopback.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                listOf(
                    "normal" to "Normal — padding only",
                    "private" to "Private — batch and jitter",
                    "high" to "High — cover traffic",
                    "maximum" to "Maximum — constant-rate cover, no calls",
                ).forEach { (id, label) ->
                    Row(
                        Modifier.fillMaxWidth().padding(top = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Text(label, modifier = Modifier.weight(1f).padding(end = 12.dp), style = MaterialTheme.typography.bodyMedium)
                        Switch(checked = privacyMode == id, onCheckedChange = { if (it) onPrivacyMode(id) })
                    }
                }
                Row(
                    Modifier.fillMaxWidth().padding(top = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween,
                ) {
                    Column(Modifier.weight(1f).padding(end = 12.dp)) {
                        Text("Read receipts", style = MaterialTheme.typography.bodyMedium)
                        Text(
                            "Send ✓✓ when you view a chat, and see others'. Off hides both ways. 1:1 chats only.",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Switch(checked = readReceiptsEnabled, onCheckedChange = onReadReceipts)
                }
            }
            item {
                Text("Notifications", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                Text(
                    "How messages reach you when the app is closed. Notification text never shows message content. " +
                        "After a reboot, unlock once to resume background sync.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                NemoNotifyMode.entries.forEach { mode ->
                    Row(
                        Modifier.fillMaxWidth().padding(top = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Column(Modifier.weight(1f).padding(end = 12.dp)) {
                            Text(mode.label, style = MaterialTheme.typography.bodyMedium)
                            Text(
                                mode.description,
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Switch(
                            checked = notifyMode == mode,
                            onCheckedChange = { if (it) onNotifyMode(mode) },
                        )
                    }
                }
                if (!notificationsAllowed) {
                    Text(
                        "System notifications are off — alerts stay silent until you allow them.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                    Button(onClick = onOpenSystemSettings, modifier = Modifier.fillMaxWidth()) {
                        Text("Open system settings")
                    }
                }
            }
            item {
                Text("Share contact", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                Button(enabled = !busy, onClick = onShare, modifier = Modifier.fillMaxWidth()) {
                    Icon(Icons.Filled.ContentCopy, null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("Copy my contact card")
                }
                shareCardBytes(shareUri)?.let { bytes ->
                    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                        ContactQr(bytes, Modifier.padding(top = 12.dp).size(200.dp))
                    }
                }
                if (shareUri.isNotEmpty()) {
                    SelectionContainer {
                        Text(shareUri, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
            item {
                Text("Group membership", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                if (selected?.isGroup == true) {
                    Button(enabled = !busy, onClick = onInviteGroup, modifier = Modifier.fillMaxWidth()) {
                        Text("Copy invite for ${selected.title}")
                    }
                }
                if (inviteUri.isNotEmpty() && selected?.isGroup == true) {
                    RelaySendPicker(
                        header = "Invite ready (also copied). Send it directly:",
                        roster = roster,
                        selectedId = inviteSendTo,
                        onSelect = onInviteSendTo,
                        contactPlaceholder = "Pick who to invite",
                        sendLabel = "Send invite via 1:1",
                        sendEnabled = !busy && inviteSendTo.isNotBlank(),
                        onSend = onSendInvite,
                    )
                }
                OutlinedTextField(
                    value = joinPaste,
                    onValueChange = onJoinPaste,
                    label = { Text("Join request (nemo-j:1:…)") },
                    modifier = Modifier.fillMaxWidth(),
                )
                Button(enabled = !busy && joinPaste.isNotBlank(), onClick = onAdmit, modifier = Modifier.fillMaxWidth()) {
                    Text("Admit to group")
                }
                if (joinUri.isNotEmpty()) {
                    SelectionContainer {
                        Text(joinUri, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                    }
                }
                if (memberCred.isNotEmpty()) {
                    Text("Last admitted $memberCred", style = MaterialTheme.typography.bodySmall)
                }
            }
            item {
                Text("Disappearing messages", style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
                OutlinedTextField(
                    value = disappearSecs,
                    onValueChange = onDisappearSecs,
                    label = { Text("Seconds (0 = off)") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    enabled = selected != null,
                )
                Button(enabled = !busy && selected != null, onClick = onDisappear, modifier = Modifier.fillMaxWidth()) {
                    Text("Apply to this chat")
                }
            }
        }
    }
}

@Composable
internal fun SheetForm(title: String, action: String, enabled: Boolean, onAction: () -> Unit, content: @Composable () -> Unit) {
    Column(
        Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp)
            .navigationBarsPadding()
            .imePadding()
            .padding(bottom = 28.dp),
    ) {
        Text(title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(12.dp))
        content()
        Spacer(Modifier.height(16.dp))
        Button(enabled = enabled, onClick = onAction, modifier = Modifier.fillMaxWidth()) { Text(action) }
    }
}

@Composable
private fun EmptyChatHint() {
    Column(
        Modifier.fillMaxSize(),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Icon(Icons.AutoMirrored.Filled.Chat, null, modifier = Modifier.size(56.dp), tint = MaterialTheme.colorScheme.outline)
        Spacer(Modifier.height(8.dp))
        Text("Select a chat", color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun Avatar(title: String, isGroup: Boolean, size: androidx.compose.ui.unit.Dp = 48.dp) {
    val palette = LocalNemoPalette.current
    // Mono recolors avatars to grayscale; blue keeps the original colorful set.
    val color = avatarColor(title, palette.mono)
    Box(
        Modifier.size(size).clip(CircleShape).background(color),
        contentAlignment = Alignment.Center,
    ) {
        if (isGroup) {
            Icon(Icons.Filled.Group, null, tint = Color.White, modifier = Modifier.size(size * 0.45f))
        } else {
            Text(
                title.trim().take(1).uppercase(),
                color = Color.White,
                fontWeight = FontWeight.SemiBold,
                fontSize = (size.value * 0.42f).sp,
            )
        }
    }
}

@Composable
private fun PassField(label: String, value: String, onChange: (String) -> Unit, onSubmit: (() -> Unit)? = null) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        visualTransformation = PasswordVisualTransformation(),
        modifier = Modifier
            .fillMaxWidth()
            .testTag(label)
            // Tell the IME / autofill this is a secret — no Gboard learning or dictionary hints.
            .semantics { password() }
            .onPreviewKeyEvent { event ->
                if (onSubmit == null) return@onPreviewKeyEvent false
                if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                if (event.key != Key.Enter && event.key != Key.NumPadEnter) return@onPreviewKeyEvent false
                onSubmit()
                true
            },
        singleLine = true,
        shape = RoundedCornerShape(12.dp),
        keyboardOptions = KeyboardOptions(
            capitalization = KeyboardCapitalization.None,
            autoCorrectEnabled = false,
            keyboardType = KeyboardType.Password,
            imeAction = if (onSubmit != null) ImeAction.Go else ImeAction.Done,
        ),
        keyboardActions = KeyboardActions(
            onGo = { onSubmit?.invoke() },
            onDone = { onSubmit?.invoke() },
        ),
    )
}

internal fun shortId(id: String) = if (id.length <= 10) id else "${id.take(6)}…"

/**
 * One-shot open jump: land the anchor bubble, then correct from the measured
 * layout until at most context rows sit above the marker. Row heights vary,
 * so index estimates alone could strand the marker off screen.
 */
@Composable
private fun JumpToUnreadEffect(
    chatId: String,
    messages: List<DisplayRow>,
    entryMark: ULong,
    entryUnreadSeq: ULong?,
    openMaxSeq: ULong,
    listState: LazyListState,
    onStickToBottom: (Boolean) -> Unit,
    onMarkEnabled: (Boolean) -> Unit,
) {
    LaunchedEffect(chatId) {
        if (entryUnreadSeq == null) {
            onMarkEnabled(true)
            return@LaunchedEffect
        }
        onStickToBottom(false)
        // Wait for a settled layout before measuring. The thread can enter
        // through an animated transition (size morphs for its duration), and
        // measuring mid-animation strands the marker and visibly drifts.
        run {
            var signature = ""
            var settled = 0
            repeat(30) {
                withFrameMillis { }
                if (listState.isScrollInProgress) {
                    // User took over before the jump; visible-max marking takes over.
                    onMarkEnabled(true)
                    return@LaunchedEffect
                }
                val info = listState.layoutInfo
                val next = "${info.viewportSize} ${info.visibleItemsInfo.size} ${info.totalItemsCount}"
                if (next == signature) {
                    settled++
                    if (settled >= 2) return@run
                } else {
                    signature = next
                    settled = 0
                }
            }
        }
        var div = unreadDividerAt(messages, chatId, entryMark, openMaxSeq)
        if (div == null) {
            onMarkEnabled(true)
            return@LaunchedEffect
        }
        val total = messages.size + 1
        val h0 = listState.layoutInfo.visibleItemsInfo.size
        if (total <= h0.coerceAtLeast(1)) {
            // Everything fits: already at the bottom with all rows on screen.
            onMarkEnabled(true)
            return@LaunchedEffect
        }
        listState.scrollToItem((div - 1).coerceAtLeast(0))
        repeat(3) {
            withFrameMillis { }
            if (listState.isScrollInProgress) {
                // User took over mid-jump; visible-max marking takes it from here.
                onMarkEnabled(true)
                return@LaunchedEffect
            }
            div = unreadDividerAt(messages, chatId, entryMark, openMaxSeq)
            if (div == null) {
                onMarkEnabled(true)
                return@LaunchedEffect
            }
            val vis = listState.layoutInfo.visibleItemsInfo.map { it.index }
            if (vis.isEmpty()) return@repeat
            val first = vis.minOrNull() ?: 0
            val top = vis.maxOrNull() ?: 0
            val low = 0
            val high = (div - 1).coerceAtLeast(0)
            if (div in first..top && top - div <= UNREAD_CONTEXT_ABOVE) {
                onMarkEnabled(true)
                return@LaunchedEffect
            }
            val target = when {
                // Marker above the screen: bring it down with context rows above it.
                div > top -> markerTopTarget(div, vis.size, UNREAD_CONTEXT_ABOVE).coerceIn(low, high)
                // Marker below the screen (list changed under us): anchor back to bottom.
                div < first -> (div - 1).coerceIn(low, high)
                // Marker on screen with too much above it: trim to context size.
                else -> (first + (top - div - UNREAD_CONTEXT_ABOVE)).coerceIn(low, high)
            }
            if (target == first) {
                onMarkEnabled(true)
                return@LaunchedEffect
            }
            listState.scrollToItem(target)
        }
        onMarkEnabled(true)
    }
}

// Visible-only read marking: the thread reports the newest visible seq,
// unseen newer rows keep their unread count when leaving.
private fun markVisibleRead(lastRead: MutableMap<String, ULong>, vaultDir: File, scope: CoroutineScope, convId: String, seq: ULong) {
    if ((lastRead[convId] ?: 0UL) >= seq) return
    lastRead[convId] = seq
    scope.launch(Dispatchers.IO) { saveUnreadSeen(vaultDir, lastRead.toMap()) }
}

/** Load the badge watermark after inbox; first unlock marks history read. */
private suspend fun loadUnreadAfterInbox(vaultDir: File, messages: List<DisplayRow>, lastRead: MutableMap<String, ULong>) {
    val persisted = withContext(Dispatchers.IO) { loadUnreadSeen(vaultDir) }
    if (persisted == null) {
        lastRead.clear()
        for (id in messages.map { it.convId }.distinct()) {
            markConversationRead(lastRead, messages, id)
        }
        withContext(Dispatchers.IO) { saveUnreadSeen(vaultDir, lastRead) }
    } else {
        lastRead.clear()
        lastRead.putAll(persisted)
    }
}

/** Wipe a pane vault so [`NemoClient.createAt`] can run again. */
internal fun wipeVaultDir(dir: File) {
    if (!dir.isDirectory) return
    dir.listFiles()?.forEach { child ->
        if (child.isDirectory) child.deleteRecursively() else child.delete()
    }
}

internal fun applyIncoming(
    messages: MutableList<DisplayRow>,
    rows: List<DisplayRow>,
    outgoing: MutableMap<String, Boolean>? = null,
    outgoingStatus: MutableMap<String, OutgoingStatus>? = null,
    /** Peer-confirmed seq per conversation (FFI `ackedUpto`). */
    ackedUpTo: (String) -> ULong = { _ -> 0UL },
    /** Peer-viewed seq per conversation (FFI `readUpto`). Zero disables. */
    readUpTo: (String) -> ULong = { _ -> 0UL },
) {
    for (row in rows) {
        if (row.kind == "call_end" || row.kind == "call_reject" || row.kind == "call_cancel") {
            stopCallAudio()
        }
        when (row.kind) {
            "removed" -> {
                messages.removeAll {
                    it.convId == row.convId &&
                        (
                            (it.convSeq == row.target && it.kind != "reaction") ||
                                (it.kind == "reaction" && it.target == row.target)
                            )
                }
                messages.removeAll {
                    it.convId == row.convId && it.kind == "deleted" && it.target == row.target
                }
                for (i in messages.indices) {
                    val m = messages[i]
                    if (m.convId == row.convId && m.replyTo == row.target && row.target != 0UL) {
                        messages[i] = m.copy(replyTo = 0UL)
                    }
                }
                // Drop any optimistic pending rows for the deleted seq.
                continue
            }
            "reaction_removed" -> {
                val idx = messages.indexOfFirst {
                    it.convId == row.convId &&
                        it.kind == "reaction" &&
                        it.target == row.target &&
                        it.emoji == row.emoji &&
                        (if (row.outgoing) it.outgoing else !it.outgoing)
                }
                if (idx >= 0) messages.removeAt(idx)
                continue
            }
            "deleted" -> {
                // Legacy tombstone: migrate to hard delete.
                messages.removeAll {
                    it.convId == row.convId &&
                        (
                            (it.convSeq == row.target && it.kind != "reaction") ||
                                (it.kind == "reaction" && it.target == row.target)
                            )
                }
                for (i in messages.indices) {
                    val m = messages[i]
                    if (m.convId == row.convId && m.replyTo == row.target && row.target != 0UL) {
                        messages[i] = m.copy(replyTo = 0UL)
                    }
                }
                continue
            }
            "expired" -> {
                val idx = messages.indexOfFirst {
                    it.convId == row.convId && it.convSeq == row.target && it.kind != "reaction"
                }
                if (idx >= 0) {
                    val old = messages[idx]
                    messages[idx] = old.copy(hidden = true, kind = row.kind, text = "", fileBytes = byteArrayOf())
                }
            }
            "reaction" -> {
                val already = messages.any {
                    it.convId == row.convId &&
                        it.kind == "reaction" &&
                        it.target == row.target &&
                        it.emoji == row.emoji &&
                        it.outgoing == row.outgoing &&
                        (it.outgoing || it.senderId == row.senderId)
                }
                if (already) continue
            }
        }
        if (row.kind != "removed" && row.kind != "reaction_removed" && row.kind != "deleted") {
            if (messages.none { it.convId == row.convId && it.convSeq == row.convSeq && it.kind == row.kind }) {
                messages.add(row)
            }
        }
        if (row.outgoing && outgoing != null) {
            val key = outgoingMapKey(row)
            outgoing[key] = true
            if (outgoingStatus != null && outgoingStatus[key] == null) {
                outgoingStatus[key] = OutgoingStatus.Sent
            }
        }
    }
    if (outgoingStatus != null) {
        // Real receipts: Sent -> Delivered (peer fetched, ✓✓ faded) ->
        // Read (peer viewed, ✓✓ full). Runs on every batch because acks and
        // read marks arrive inside fetches, not as events. One lookup per
        // conversation, cached per pass.
        val confirmed = mutableMapOf<String, ULong>()
        val viewed = mutableMapOf<String, ULong>()
        for (m in messages) {
            if (!m.outgoing || m.convSeq == 0UL) continue
            val key = outgoingMapKey(m)
            if (outgoingStatus[key] == OutgoingStatus.Read) continue
            val upto = confirmed.getOrPut(m.convId) {
                runCatching { ackedUpTo(m.convId) }.getOrDefault(0UL)
            }
            if (m.convSeq <= upto && outgoingStatus[key] != OutgoingStatus.Delivered) {
                outgoingStatus[key] = OutgoingStatus.Delivered
            }
            val seen = viewed.getOrPut(m.convId) {
                runCatching { readUpTo(m.convId) }.getOrDefault(0UL)
            }
            if (m.convSeq <= seen) outgoingStatus[key] = OutgoingStatus.Read
        }
    }
}

internal fun shareCardBytes(uri: String): ByteArray? {
    if (!uri.startsWith("nemo:1:")) return null
    val hex = uri.removePrefix("nemo:1:").substringBefore('#')
    if (hex.length < 2 || hex.length % 2 != 0) return null
    return try {
        hex.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    } catch (_: Throwable) {
        null
    }
}

@Composable
private fun ContactQr(bytes: ByteArray, modifier: Modifier = Modifier) {
    val matrix = remember(bytes) { encodeContactQr(bytes) }
    Canvas(modifier) {
        val n = matrix.width
        if (n == 0) return@Canvas
        val cell = size.minDimension / n
        drawRect(Color.White)
        for (y in 0 until n) {
            for (x in 0 until n) {
                if (matrix.get(x, y)) {
                    drawRect(
                        color = Color.Black,
                        topLeft = Offset(x * cell, y * cell),
                        size = Size(cell, cell),
                    )
                }
            }
        }
    }
}
