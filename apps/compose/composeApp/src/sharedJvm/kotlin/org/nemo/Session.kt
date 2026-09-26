package org.nemo

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.EaseOutCubic
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
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
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Chat
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.AccessTime
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Call
import androidx.compose.material.icons.filled.CallEnd
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.nemo.DisplayRow
import uniffi.nemo.NemoClient
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.atomic.AtomicLong

internal const val CANNOT_RECOVER =
    "This identity cannot be recovered or exported. If you lose the passphrase, the keys and history are gone."

/** Quiet create-screen footnote — full policy stays in [CANNOT_RECOVER]. */
internal const val CREATE_FOOTNOTE = "No recovery if you lose this passphrase."

internal const val UNLOCK_FOOTNOTE = "There is no recovery if the passphrase is wrong."

internal fun canSaveAttachment(row: DisplayRow): Boolean = row.fileName.isNotEmpty() && row.fileBytes.isNotEmpty() && !row.hidden

private enum class Phase { Locked, Create, Mnemonic, Home }

private enum class Sheet { None, AddContact, NewGroup, JoinGroup }

private data class ChatTarget(val id: String, val title: String, val isGroup: Boolean)

/** Ticks for outgoing bubbles (peer read receipts are off by default). */
internal enum class OutgoingStatus {
    Pending,
    Sent,
    Delivered,
    Failed,
}

private val localMsgSeq = AtomicLong(0)

internal fun messageListKey(row: DisplayRow): String = if (row.fetchToken.startsWith("local:")) {
    row.fetchToken
} else {
    "${row.convId}:${row.convSeq}:${row.kind}:${row.target}"
}

internal fun outgoingMapKey(row: DisplayRow): String = if (row.fetchToken.startsWith("local:")) {
    row.fetchToken
} else {
    "${row.convId}:${row.convSeq}:${row.kind}"
}

/** Composer → bubble flight: mobile only. See [MessageSendAnimation]. */

/** Map [child] window bounds into [parent]'s local coordinates (parent need not be an ancestor). */
private fun boundsInParent(parent: LayoutCoordinates, child: LayoutCoordinates): Rect {
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

private fun optimisticTextRow(chatId: String, text: String, localId: String): DisplayRow = DisplayRow(
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
)

private fun optimisticFileRow(chatId: String, fileName: String, bytes: ByteArray, localId: String): DisplayRow = DisplayRow(
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
)

private val AvatarPalette = listOf(
    Color(0xFF3390EC),
    Color(0xFF0369A1),
    Color(0xFF7C3AED),
    Color(0xFFB45309),
    Color(0xFFBE185D),
    Color(0xFF15803D),
    Color(0xFF1D4ED8),
    Color(0xFF0E7490),
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
    var cardPaste by remember { mutableStateOf("") }
    var cardPreviewFp by remember { mutableStateOf("") }
    var nickname by remember { mutableStateOf("") }
    var groupName by remember { mutableStateOf("") }
    var invitePaste by remember { mutableStateOf("") }
    var joinUri by remember { mutableStateOf("") }
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
    val contacts = remember { mutableStateMapOf<String, String>() }
    val groups = remember { mutableStateMapOf<String, String>() }

    fun runIo(block: suspend () -> Unit) {
        if (busy) return
        busy = true
        scope.launch {
            try {
                block()
            } catch (e: Throwable) {
                snackbar.showSnackbar(e.message ?: e.toString())
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

    LaunchedEffect(client, phase) {
        val c = client ?: return@LaunchedEffect
        if (phase != Phase.Home) return@LaunchedEffect
        while (true) {
            delay(2_000)
            try {
                val rows = withContext(Dispatchers.IO) { c.fetchNow() }
                applyIncoming(messages, rows, outgoing, outgoingStatus)
                val expired = withContext(Dispatchers.IO) { c.expireNow() }
                applyIncoming(messages, expired, outgoing, outgoingStatus)
                val contactRows = withContext(Dispatchers.IO) { c.listContacts() }
                val groupRows = withContext(Dispatchers.IO) { c.listGroups() }
                contacts.clear()
                contactRows.forEach { contacts[it.identityId] = it.nickname }
                groups.clear()
                groupRows.forEach { groups[it.groupId] = it.nickname.ifBlank { "Group" } }
            } catch (_: Throwable) {
            }
        }
    }

    LaunchedEffect(client, phase) {
        val c = client ?: return@LaunchedEffect
        if (phase != Phase.Home) return@LaunchedEffect
        while (true) {
            try {
                withContext(Dispatchers.IO) { c.waitWakeup() }
                val rows = withContext(Dispatchers.IO) { c.fetchNow() }
                applyIncoming(messages, rows, outgoing, outgoingStatus)
            } catch (_: Throwable) {
                delay(2_000)
            }
        }
    }

    Surface(modifier) {
        Box(Modifier.fillMaxSize().imePadding()) {
            Column(Modifier.fillMaxSize()) {
                when (phase) {
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
                            ) { Text("Create identity") }
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
                                    fingerprint = withContext(Dispatchers.IO) { c.fingerprint() }
                                    identityHex = withContext(Dispatchers.IO) { c.identityIdHex() }
                                    withContext(Dispatchers.IO) { reloadRoster(c) }
                                    applyIncoming(
                                        messages,
                                        withContext(Dispatchers.IO) { c.inbox() },
                                        outgoing,
                                        outgoingStatus,
                                    )
                                    privacyMode = withContext(Dispatchers.IO) { c.privacyMode() }
                                    registered = true
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
                            ) { Text("Unlock") }
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
                                            client = null
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
                            footer = {
                                TextButton(
                                    onClick = {
                                        val phrase = mnemonic.orEmpty()
                                        if (phrase.isNotEmpty()) {
                                            copyToClipboard(phrase)
                                            scope.launch { snackbar.showSnackbar("Copied") }
                                        }
                                    },
                                ) { Text("Copy phrase") }
                            },
                        ) {
                            SelectionContainer {
                                Column(
                                    Modifier.fillMaxWidth(),
                                    verticalArrangement = Arrangement.spacedBy(8.dp),
                                ) {
                                    words.chunked(3).forEachIndexed { rowIdx, rowWords ->
                                        Row(
                                            Modifier.fillMaxWidth(),
                                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                                        ) {
                                            rowWords.forEachIndexed { colIdx, word ->
                                                val index = rowIdx * 3 + colIdx + 1
                                                Text(
                                                    "$index  $word",
                                                    style = MaterialTheme.typography.bodyMedium,
                                                    fontFamily = FontFamily.Monospace,
                                                    color = MaterialTheme.colorScheme.onSurface,
                                                    modifier = Modifier
                                                        .weight(1f)
                                                        .clip(RoundedCornerShape(10.dp))
                                                        .background(chipBg)
                                                        .padding(horizontal = 10.dp, vertical = 10.dp),
                                                )
                                            }
                                            repeat(3 - rowWords.size) {
                                                Spacer(Modifier.weight(1f))
                                            }
                                        }
                                    }
                                }
                            }
                            Spacer(Modifier.height(20.dp))
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
                        var micAction by remember { mutableStateOf<(() -> Unit)?>(null) }
                        val requestMic = rememberEnsureMic { micAction?.invoke() }
                        BoxWithConstraints(Modifier.fillMaxSize()) {
                            val split = maxWidth >= 720.dp
                            val chat = selected
                            // System / edge-swipe back: settings → chat list → leave app.
                            NemoBackHandler(enabled = showSettings || selected != null) {
                                when {
                                    showSettings -> showSettings = false
                                    selected != null -> selected = null
                                }
                            }
                            when {
                                showSettings -> {
                                    LaunchedEffect(Unit) {
                                        runIo {
                                            shareUri = withContext(Dispatchers.IO) {
                                                c?.mintShareUri().orEmpty()
                                            }
                                        }
                                    }
                                    LaunchedEffect(chat?.id) {
                                        if (chat != null && !chat.isGroup) {
                                            contactNickname = contacts[chat.id] ?: chat.title
                                        }
                                    }
                                    SettingsScreen(
                                        fingerprint = fingerprint,
                                        identityHex = identityHex,
                                        homeUrl = homeUrl,
                                        onHomeUrl = { homeUrl = it },
                                        shareUri = shareUri,
                                        joinUri = joinUri,
                                        joinPaste = joinPaste,
                                        onJoinPaste = { joinPaste = it },
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
                                                withContext(Dispatchers.IO) { c?.register(homeUrl.trim()) }
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
                                                copyToClipboard(uri)
                                                snackbar.showSnackbar("Invite copied")
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
                                    )
                                }
                                split -> Row(Modifier.fillMaxSize()) {
                                    ChatListPane(
                                        label = label,
                                        chats = chats(),
                                        messages = messages,
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
                                    )
                                    VerticalDivider()
                                    Box(Modifier.weight(1f).fillMaxHeight()) {
                                        if (chat == null) {
                                            EmptyChatHint()
                                        } else {
                                            ChatThread(
                                                chat = chat,
                                                messages = messages.filter { it.convId == chat.id },
                                                outgoing = outgoing,
                                                outgoingStatus = outgoingStatus,
                                                draft = draft,
                                                onDraft = { draft = it },
                                                showBack = false,
                                                onBack = { selected = null },
                                                onSettings = { showSettings = true },
                                                chatMenu = chatMenu,
                                                onChatMenu = { chatMenu = it },
                                                onSend = {
                                                    sendChat(
                                                        c,
                                                        chat,
                                                        draft,
                                                        messages,
                                                        outgoing,
                                                        outgoingStatus,
                                                        scope,
                                                        snackbar,
                                                    ) { draft = "" }
                                                },
                                                onAttach = { pickFile() },
                                                onCall = {
                                                    micAction = {
                                                        runIo {
                                                            val row = withContext(Dispatchers.IO) {
                                                                c?.startCall(chat.id)
                                                            } ?: return@runIo
                                                            messages.add(row)
                                                            markOutgoing(row)
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
                                                    runIo {
                                                        val r = withContext(Dispatchers.IO) {
                                                            c?.react(chat.id, row.convSeq, "👍")
                                                        } ?: return@runIo
                                                        messages.add(r)
                                                        markOutgoing(r)
                                                    }
                                                },
                                                onDelete = { row ->
                                                    runIo {
                                                        val r = withContext(Dispatchers.IO) {
                                                            c?.deleteMessage(chat.id, row.convSeq)
                                                        } ?: return@runIo
                                                        applyIncoming(messages, listOf(r), outgoing, outgoingStatus)
                                                    }
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
                                            )
                                        }
                                    }
                                }
                                chat != null -> ChatThread(
                                    chat = chat,
                                    messages = messages.filter { it.convId == chat.id },
                                    outgoing = outgoing,
                                    outgoingStatus = outgoingStatus,
                                    draft = draft,
                                    onDraft = { draft = it },
                                    showBack = true,
                                    onBack = { selected = null },
                                    onSettings = { showSettings = true },
                                    chatMenu = chatMenu,
                                    onChatMenu = { chatMenu = it },
                                    onSend = {
                                        sendChat(
                                            c,
                                            chat,
                                            draft,
                                            messages,
                                            outgoing,
                                            outgoingStatus,
                                            scope,
                                            snackbar,
                                        ) { draft = "" }
                                    },
                                    onAttach = { pickFile() },
                                    onCall = {
                                        micAction = {
                                            runIo {
                                                val row = withContext(Dispatchers.IO) {
                                                    c?.startCall(chat.id)
                                                } ?: return@runIo
                                                messages.add(row)
                                                markOutgoing(row)
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
                                        runIo {
                                            val r = withContext(Dispatchers.IO) {
                                                c?.react(chat.id, row.convSeq, "👍")
                                            } ?: return@runIo
                                            messages.add(r)
                                            markOutgoing(r)
                                        }
                                    },
                                    onDelete = { row ->
                                        runIo {
                                            val r = withContext(Dispatchers.IO) {
                                                c?.deleteMessage(chat.id, row.convSeq)
                                            } ?: return@runIo
                                            applyIncoming(messages, listOf(r), outgoing, outgoingStatus)
                                        }
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
                                )
                                else -> ChatListPane(
                                    label = label,
                                    chats = chats(),
                                    messages = messages,
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
                                )
                            }
                        }
                        if (sheet != Sheet.None) {
                            ModalBottomSheet(
                                onDismissRequest = { sheet = Sheet.None },
                                sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
                            ) {
                                when (sheet) {
                                    Sheet.AddContact -> {
                                        val scanQr = rememberScanQr { text ->
                                            cardPaste = text.trim()
                                        }
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
                                            onAction = {
                                                runIo {
                                                    val peer = withContext(Dispatchers.IO) {
                                                        c?.addContact(
                                                            cardPaste.trim(),
                                                            nickname.ifBlank { "Contact" },
                                                        ).orEmpty()
                                                    }
                                                    contacts[peer] = nickname.ifBlank { shortId(peer) }
                                                    selected = ChatTarget(peer, contacts[peer] ?: shortId(peer), false)
                                                    cardPaste = ""
                                                    cardPreviewFp = ""
                                                    nickname = ""
                                                    sheet = Sheet.None
                                                }
                                            },
                                        ) {
                                            OutlinedTextField(
                                                value = cardPaste,
                                                onValueChange = { cardPaste = it },
                                                label = { Text("Contact card") },
                                                placeholder = { Text("nemo:1:…") },
                                                modifier = Modifier.fillMaxWidth(),
                                            )
                                            TextButton(onClick = scanQr) {
                                                Text("Scan QR")
                                            }
                                            if (cardPreviewFp.isNotEmpty()) {
                                                Text(
                                                    "Fingerprint",
                                                    style = MaterialTheme.typography.labelSmall,
                                                )
                                                SelectionContainer {
                                                    Text(
                                                        cardPreviewFp,
                                                        fontFamily = FontFamily.Monospace,
                                                        style = MaterialTheme.typography.bodySmall,
                                                    )
                                                }
                                            }
                                            OutlinedTextField(
                                                value = nickname,
                                                onValueChange = { nickname = it },
                                                label = { Text("Name") },
                                                modifier = Modifier.fillMaxWidth(),
                                                singleLine = true,
                                            )
                                        }
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
                                                sheet = Sheet.None
                                                snackbar.showSnackbar("Join request copied — send it to a member")
                                            }
                                        },
                                    ) {
                                        OutlinedTextField(
                                            value = invitePaste,
                                            onValueChange = { invitePaste = it },
                                            label = { Text("Group invite") },
                                            placeholder = { Text("nemo-g:1:…") },
                                            modifier = Modifier.fillMaxWidth(),
                                        )
                                    }
                                    Sheet.None -> {}
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
) {
    val text = draft.trim()
    if (text.isEmpty()) return
    clear()
    val localId = newLocalId()
    val pending = optimisticTextRow(chat.id, text, localId)
    messages.add(pending)
    outgoing[localId] = true
    outgoingStatus[localId] = OutgoingStatus.Pending
    scope.launch {
        try {
            val row = withContext(Dispatchers.IO) {
                if (chat.isGroup) {
                    client?.sendGroupText(chat.id, text)
                } else {
                    client?.sendText(chat.id, text)
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
            // Network send finished. ✓✓ until real protocol_ack lands in the shell.
            outgoingStatus[localId] = OutgoingStatus.Delivered
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
            outgoingStatus[localId] = OutgoingStatus.Delivered
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
    val dark = nemoDarkTheme()
    val atmosphere = Brush.verticalGradient(
        colors = if (dark) NemoOnboardGradientDark else NemoOnboardGradientLight,
    )
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
) {
    val dark = nemoDarkTheme()
    val listBg = if (dark) NemoChatDark else NemoListLight
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
                colors = TopAppBarDefaults.centerAlignedTopAppBarColors(
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
        if (chats.isEmpty()) {
            Column(
                Modifier
                    .fillMaxSize()
                    .padding(padding)
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
            LazyColumn(Modifier.fillMaxSize().padding(padding)) {
                items(chats, key = { it.id }) { chat ->
                    val last = messages.lastOrNull { it.convId == chat.id }
                    ChatRow(
                        chat = chat,
                        preview = last?.let { previewLine(it) } ?: "No messages yet",
                        time = last?.let { formatTime(it.sentAt) }.orEmpty(),
                        selected = chat.id == selectedId,
                        onClick = { onSelect(chat) },
                    )
                }
            }
        }
    }
}

@Composable
private fun ChatRow(chat: ChatTarget, preview: String, time: String, selected: Boolean, onClick: () -> Unit) {
    val bg = when {
        selected -> MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.55f)
        else -> Color.Transparent
    }
    val hairline = MaterialTheme.colorScheme.outline.copy(alpha = 0.35f)
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
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    if (time.isNotEmpty()) {
                        Text(
                            time,
                            style = MaterialTheme.typography.labelSmall,
                            color = if (selected) {
                                MaterialTheme.colorScheme.primary
                            } else {
                                MaterialTheme.colorScheme.onSurfaceVariant
                            },
                        )
                    }
                }
                Spacer(Modifier.height(2.dp))
                Text(
                    preview,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
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
) {
    val listState = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val dark = nemoDarkTheme()
    val wallpaper = if (dark) NemoChatDark else NemoChatLight
    val canSend = draft.isNotBlank()
    val sendScale by animateFloatAsState(
        targetValue = if (canSend) 1f else 0.88f,
        animationSpec = spring(dampingRatio = Spring.DampingRatioMediumBouncy, stiffness = Spring.StiffnessMediumLow),
        label = "sendScale",
    )
    var stickToBottom by remember(chat.id) { mutableStateOf(true) }
    val lastKey = messages.lastOrNull()?.let { messageListKey(it) }
    val knownKeys = remember(chat.id) { mutableSetOf<String>() }
    var seedDone by remember(chat.id) { mutableStateOf(false) }

    var overlayRoot by remember { mutableStateOf<LayoutCoordinates?>(null) }
    var composerCoords by remember { mutableStateOf<LayoutCoordinates?>(null) }
    var overlayWindowOrigin by remember { mutableStateOf(Offset.Zero) }
    var rootHeightPx by remember { mutableFloatStateOf(1f) }
    var pendingFly by remember(chat.id) { mutableStateOf<PendingMessageSend?>(null) }
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
    LaunchedEffect(listState) {
        snapshotFlow {
            val info = listState.layoutInfo
            // reverseLayout: index 0 is the newest message at the bottom (above composer).
            val first = info.visibleItemsInfo.firstOrNull() ?: return@snapshotFlow true
            first.index <= 1
        }.distinctUntilChanged().collect { nearBottom ->
            stickToBottom = nearBottom
        }
    }
    var chatReady by remember(chat.id) { mutableStateOf(false) }
    LaunchedEffect(chat.id) {
        chatReady = false
        withFrameMillis { }
        chatReady = true
    }
    LaunchedEffect(lastKey, chat.id) {
        if (messages.isEmpty() || !stickToBottom) return@LaunchedEffect
        // Already pinned: reverseLayout inserts at index 0 without scrolling.
        // Calling scrollToItem again after insert is what makes the list "shake" once.
        val alreadyPinned =
            listState.firstVisibleItemIndex == 0 && listState.firstVisibleItemScrollOffset == 0
        if (alreadyPinned && chatReady) return@LaunchedEffect
        listState.animateChatToBottom(animated = chatReady)
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
                listState.animateChatToBottom(animated = true)
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
                        modifier = Modifier.weight(1f),
                        shape = pillShape,
                        color = pillColor,
                        shadowElevation = 2.dp,
                        tonalElevation = 0.dp,
                        onClick = onSettings,
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
                Surface(
                    color = MaterialTheme.colorScheme.surface.copy(alpha = 0.96f),
                    shadowElevation = 6.dp,
                ) {
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .navigationBarsPadding()
                            .imePadding()
                            .padding(horizontal = 6.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.Bottom,
                    ) {
                        IconButton(onClick = onAttach) {
                            Icon(
                                Icons.Filled.AttachFile,
                                contentDescription = "Attach",
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Surface(
                            modifier = Modifier
                                .weight(1f)
                                .onGloballyPositioned { composerCoords = it },
                            shape = RoundedCornerShape(22.dp),
                            color = MaterialTheme.colorScheme.surfaceVariant.copy(alpha = if (dark) 0.55f else 0.85f),
                            tonalElevation = 0.dp,
                        ) {
                            BasicTextField(
                                value = draft,
                                onValueChange = onDraft,
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .heightIn(min = 44.dp)
                                    .padding(horizontal = 16.dp, vertical = 12.dp)
                                    .onPreviewKeyEvent { event ->
                                        if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
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
                                    lineHeight = 22.sp,
                                ),
                                cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                                maxLines = 5,
                                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                                keyboardActions = KeyboardActions(onSend = { requestSend() }),
                                decorationBox = { inner ->
                                    Box {
                                        if (draft.isEmpty()) {
                                            Text(
                                                "Message",
                                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                                fontSize = 16.sp,
                                            )
                                        }
                                        inner()
                                    }
                                },
                            )
                        }
                        Spacer(Modifier.width(6.dp))
                        FilledIconButton(
                            onClick = requestSend,
                            enabled = canSend,
                            modifier = Modifier.scale(sendScale).size(46.dp),
                            shape = CircleShape,
                        ) {
                            Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Send")
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
                ChatWallpaper(dark = dark, modifier = Modifier.fillMaxSize())
                // reverseLayout stacks short threads on the composer (empty space above).
                val newestFirst = remember(messages) { messages.asReversed() }
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
                    itemsIndexed(
                        newestFirst,
                        key = { _, it -> messageListKey(it) },
                    ) { index, row ->
                        // Chronological neighbors for clustering (list is newest-first).
                        val chronoIndex = messages.lastIndex - index
                        val key = outgoingMapKey(row)
                        val mine = row.outgoing || outgoing[key] == true
                        val prevMine = messages.getOrNull(chronoIndex - 1)?.let { prev ->
                            prev.outgoing || outgoing[outgoingMapKey(prev)] == true
                        }
                        val nextMine = messages.getOrNull(chronoIndex + 1)?.let { next ->
                            next.outgoing || outgoing[outgoingMapKey(next)] == true
                        }
                        val clusteredAbove = prevMine == mine
                        val clusteredBelow = nextMine == mine
                        val gap = if (clusteredAbove) 2.dp else 8.dp
                        val listKey = messageListKey(row)
                        val isFlying = useFlyMorph && listKey in flyingKeys
                        val animateEnter = remember(listKey) {
                            val neu = seedDone && listKey !in knownKeys
                            if (neu) knownKeys.add(listKey)
                            neu
                        }
                        MessageBubble(
                            row = row,
                            mine = mine,
                            status = if (mine) outgoingStatus[key] else null,
                            clusteredAbove = clusteredAbove,
                            clusteredBelow = clusteredBelow,
                            animateEnter = animateEnter && !isFlying,
                            conceal = isFlying,
                            onBubbleCoords = if (isFlying) {
                                { coords ->
                                    overlayRoot?.let { parent ->
                                        flyTargets[listKey] = boundsInParent(parent, coords)
                                    }
                                }
                            } else {
                                null
                            },
                            onReact = { onReact(row) },
                            onDelete = { onDelete(row) },
                            onSave = { onSave(row) },
                            // No animateItem: with reverseLayout, a new message shifts every
                            // visible index and placement animation makes the thread shake.
                            modifier = Modifier.padding(top = gap),
                        )
                    }
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
private fun ChatWallpaper(dark: Boolean, modifier: Modifier = Modifier) {
    val base = if (dark) NemoChatDark else NemoChatLight
    val dot = if (dark) NemoPatternDotDark else NemoPatternDotLight
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
private fun MessageBubble(
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
    val outgoingBrush = if (mine) {
        outgoingScreenBrush(dark, windowY, rootHeight)
    } else {
        null
    }
    val incomingBg = if (dark) NemoIncomingDark else NemoIncomingLight
    val corner = 18.dp
    val tight = 6.dp
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
    val shadow = if (dark) NemoBubbleShadowDark else NemoBubbleShadowLight
    val metaColor = if (mine && dark) {
        Color(0xFFB8D4E8).copy(alpha = 0.9f)
    } else {
        MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.85f)
    }
    val bodyColor = when {
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
            Column(
                Modifier
                    .widthIn(max = 320.dp)
                    .onGloballyPositioned { coords ->
                        onBubbleCoords?.invoke(coords)
                        if (mine) {
                            windowY = coords.positionInWindow().y
                            rootHeight = coords.findRootCoordinates().size.height.toFloat()
                        }
                    }
                    .shadow(2.dp, shape, ambientColor = shadow, spotColor = shadow)
                    .clip(shape)
                    .then(
                        if (outgoingBrush != null) {
                            Modifier.background(outgoingBrush)
                        } else {
                            Modifier.background(incomingBg)
                        },
                    )
                    .clickable(enabled = !conceal) { menu = true }
                    .padding(horizontal = 12.dp, vertical = 7.dp),
            ) {
                Text(
                    bubbleText(row),
                    style = MaterialTheme.typography.bodyLarge,
                    color = bodyColor,
                )
                Row(
                    Modifier.align(Alignment.End).padding(top = 2.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(3.dp),
                ) {
                    Text(
                        formatTime(row.sentAt),
                        style = MaterialTheme.typography.labelSmall,
                        color = metaColor,
                    )
                    if (mine && status != null) {
                        DeliveryTicks(status = status, tint = metaColor)
                    }
                }
            }
            DropdownMenu(
                expanded = menu,
                onDismissRequest = { menu = false },
                containerColor = MaterialTheme.colorScheme.surface,
                tonalElevation = 0.dp,
                shadowElevation = 8.dp,
            ) {
                if (canSaveAttachment(row)) {
                    DropdownMenuItem(text = { Text("Save") }, onClick = {
                        menu = false
                        onSave()
                    })
                }
                DropdownMenuItem(text = { Text("React 👍") }, onClick = {
                    menu = false
                    onReact()
                })
                DropdownMenuItem(text = { Text("Delete") }, onClick = {
                    menu = false
                    onDelete()
                })
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
        OutgoingStatus.Failed -> Icons.Filled.ErrorOutline to "Failed"
    }
    val color = when (status) {
        OutgoingStatus.Failed -> MaterialTheme.colorScheme.error
        OutgoingStatus.Delivered -> MaterialTheme.colorScheme.primary
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
                    "Light and dark use Nemo’s blue chat theme. System follows the OS.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                val themeMode = LocalThemeMode.current
                val onThemeMode = LocalOnThemeModeChange.current
                Row(
                    Modifier.fillMaxWidth().padding(top = 4.dp),
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
private fun SheetForm(title: String, action: String, enabled: Boolean, onAction: () -> Unit, content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(horizontal = 20.dp).padding(bottom = 28.dp)) {
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
    val color = AvatarPalette[kotlin.math.abs(title.hashCode()) % AvatarPalette.size]
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

private fun shortId(id: String) = if (id.length <= 10) id else "${id.take(6)}…"

/** Wipe a pane vault so [`NemoClient.createAt`] can run again. */
internal fun wipeVaultDir(dir: File) {
    if (!dir.isDirectory) return
    dir.listFiles()?.forEach { child ->
        if (child.isDirectory) child.deleteRecursively() else child.delete()
    }
}

internal fun previewLine(row: DisplayRow): String = when {
    row.hidden && row.kind == "expired" -> "Message expired"
    row.hidden || row.kind == "deleted" -> "Message deleted"
    row.kind == "reaction" -> "Reacted ${row.emoji}"
    row.kind == "call_invite" -> "Incoming call"
    row.kind == "call_ringing" -> "Ringing"
    row.kind == "call_answer" -> "Call answered"
    row.kind == "call_reject" -> "Declined"
    row.kind == "call_cancel" -> "Cancelled"
    row.kind == "call_end" -> "Call ended"
    row.kind == "lost" -> "Messages lost"
    row.kind == "revoked" -> "Identity revoked"
    row.kind == "binding_conflict" -> "Home-server binding conflict"
    row.fileName.isNotEmpty() -> "📎 ${row.fileName}"
    else -> row.text
}

internal fun bubbleText(row: DisplayRow): String = when {
    row.hidden && row.kind == "expired" -> "Expired"
    row.hidden || row.kind == "deleted" -> "Deleted"
    row.kind == "reaction" -> "Reacted ${row.emoji}"
    row.kind == "disappear" -> "Disappearing messages: ${row.text}s"
    row.kind == "call_invite" -> "Incoming call"
    row.kind == "call_ringing" -> "Ringing"
    row.kind == "call_answer" -> "Answered"
    row.kind == "call_reject" -> "Declined"
    row.kind == "call_cancel" -> "Cancelled"
    row.kind == "call_end" -> "Call ended"
    row.kind == "lost" -> "Messages lost"
    row.kind == "revoked" -> "Identity revoked"
    row.kind == "binding_conflict" -> "Home-server binding conflict"
    row.fileName.isNotEmpty() -> "📎 ${row.fileName}"
    else -> row.text
}

private fun formatTime(sentAt: ULong): String {
    if (sentAt == 0UL) return ""
    return try {
        SimpleDateFormat("HH:mm", Locale.getDefault()).format(Date(sentAt.toLong() * 1000))
    } catch (_: Throwable) {
        ""
    }
}

internal fun applyIncoming(
    messages: MutableList<DisplayRow>,
    rows: List<DisplayRow>,
    outgoing: MutableMap<String, Boolean>? = null,
    outgoingStatus: MutableMap<String, OutgoingStatus>? = null,
) {
    for (row in rows) {
        if (row.kind == "call_end" || row.kind == "call_reject" || row.kind == "call_cancel") {
            stopCallAudio()
        }
        if (row.kind == "deleted" || row.kind == "expired") {
            val idx = messages.indexOfFirst {
                it.convId == row.convId && it.convSeq == row.target && it.kind != "reaction"
            }
            if (idx >= 0) {
                val old = messages[idx]
                messages[idx] = old.copy(hidden = true, kind = row.kind, text = "", fileBytes = byteArrayOf())
            }
        }
        if (messages.none { it.convId == row.convId && it.convSeq == row.convSeq && it.kind == row.kind }) {
            messages.add(row)
        }
        if (row.outgoing && outgoing != null) {
            val key = outgoingMapKey(row)
            outgoing[key] = true
            if (outgoingStatus != null && outgoingStatus[key] == null) {
                outgoingStatus[key] = OutgoingStatus.Delivered
            }
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
