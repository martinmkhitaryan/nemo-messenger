package org.nemo

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
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
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Chat
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.AttachFile
import androidx.compose.material.icons.filled.Call
import androidx.compose.material.icons.filled.CallEnd
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Group
import androidx.compose.material.icons.filled.Lock
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
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.VerticalDivider
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.nemo.DisplayRow
import uniffi.nemo.NemoClient
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

internal const val CANNOT_RECOVER =
    "This identity cannot be recovered or exported. If you lose the passphrase, the keys and history are gone."

expect fun pickLocalFile(): String?

expect fun deviceVaultSecret(vaultDir: File): ByteArray

@Composable
expect fun rememberPickFile(onPicked: (String) -> Unit): () -> Unit

@Composable
expect fun rememberScanQr(onText: (String) -> Unit): () -> Unit

expect fun defaultHomeUrl(): String

expect fun copyToClipboard(text: String)

expect fun startCallAudio(client: NemoClient)

expect fun stopCallAudio()

@Composable
expect fun rememberEnsureMic(onReady: () -> Unit): () -> Unit

private enum class Phase { Locked, Create, Mnemonic, Home }

private enum class Sheet { None, AddContact, NewGroup, JoinGroup }

private data class ChatTarget(
    val id: String,
    val title: String,
    val isGroup: Boolean,
)

private val AvatarPalette = listOf(
    Color(0xFF0F766E),
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

    fun markOutgoing(row: DisplayRow) {
        outgoing["${row.convId}:${row.convSeq}:${row.kind}"] = true
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
                applyIncoming(messages, rows)
                val expired = withContext(Dispatchers.IO) { c.expireNow() }
                applyIncoming(messages, expired)
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
                applyIncoming(messages, rows)
            } catch (_: Throwable) {
                delay(2_000)
            }
        }
    }

    Surface(modifier) {
        Column(Modifier.fillMaxSize().imePadding()) {
            SnackbarHost(snackbar)
            when (phase) {
                Phase.Create -> OnboardScaffold(
                    title = "Create identity",
                    subtitle = CANNOT_RECOVER,
                ) {
                    PassField("Passphrase (min 8)", passphrase) { passphrase = it }
                    PassField("Confirm", confirm) { confirm = it }
                    Spacer(Modifier.height(12.dp))
                    Button(
                        enabled = !busy,
                        modifier = Modifier.fillMaxWidth().testTag("create-identity"),
                        onClick = {
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
                                        deviceVaultSecret(vaultDir),
                                    )
                                }
                                mnemonic = withContext(Dispatchers.IO) { c.takeRevocationMnemonic() }
                                client = c
                                phase = Phase.Mnemonic
                            }
                        },
                    ) { Text("Create identity") }
                    if (File(vaultDir, "kdf.cbor").isFile) {
                        TextButton(onClick = { phase = Phase.Locked }) { Text("Unlock existing instead") }
                    }
                }
                Phase.Locked -> OnboardScaffold(
                    title = "Welcome back",
                    subtitle = "Unlock this vault. There is no recovery if the passphrase is wrong.",
                ) {
                    PassField("Passphrase", passphrase) { passphrase = it }
                    Spacer(Modifier.height(12.dp))
                    Button(
                        enabled = !busy,
                        modifier = Modifier.fillMaxWidth(),
                        onClick = {
                            runIo {
                                val c = withContext(Dispatchers.IO) {
                                    NemoClient.openAt(
                                        vaultDir.absolutePath,
                                        passphrase,
                                        deviceVaultSecret(vaultDir),
                                    )
                                }
                                client = c
                                fingerprint = withContext(Dispatchers.IO) { c.fingerprint() }
                                identityHex = withContext(Dispatchers.IO) { c.identityIdHex() }
                                withContext(Dispatchers.IO) { reloadRoster(c) }
                                applyIncoming(messages, withContext(Dispatchers.IO) { c.inbox() })
                                privacyMode = withContext(Dispatchers.IO) { c.privacyMode() }
                                registered = true
                                phase = Phase.Home
                            }
                        },
                    ) { Text("Unlock") }
                    TextButton(onClick = { confirmWipe = true }) {
                        Text("Create a new identity")
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
                Phase.Mnemonic -> OnboardScaffold(
                    title = "Recovery phrase",
                    subtitle = "Write this revocation phrase down. It is never stored.",
                ) {
                    SelectionContainer {
                        Text(
                            mnemonic ?: "",
                            fontFamily = FontFamily.Monospace,
                            style = MaterialTheme.typography.bodyLarge,
                            modifier = Modifier
                                .fillMaxWidth()
                                .clip(RoundedCornerShape(12.dp))
                                .background(MaterialTheme.colorScheme.surfaceVariant)
                                .padding(16.dp),
                        )
                    }
                    Spacer(Modifier.height(12.dp))
                    Button(
                        modifier = Modifier.fillMaxWidth(),
                        onClick = {
                            runIo {
                                val c = client ?: return@runIo
                                fingerprint = withContext(Dispatchers.IO) { c.fingerprint() }
                                identityHex = withContext(Dispatchers.IO) { c.identityIdHex() }
                                showSettings = true
                                phase = Phase.Home
                            }
                        },
                    ) { Text("I wrote it down") }
                }
                Phase.Home -> {
                    val c = client
                    val pickFile = rememberPickFile { path ->
                        val chat = selected ?: return@rememberPickFile
                        attachFilePath(c, chat, path, messages, outgoing, snackbar, runIo = { runIo(it) })
                    }
                    var micAction by remember { mutableStateOf<(() -> Unit)?>(null) }
                    val requestMic = rememberEnsureMic { micAction?.invoke() }
                    BoxWithConstraints(Modifier.fillMaxSize()) {
                        val split = maxWidth >= 720.dp
                        val chat = selected
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
                                    onSelect = { selected = it; showSettings = false },
                                    onSettings = { showSettings = true },
                                    onAdd = { addMenu = true },
                                    addMenu = addMenu,
                                    onAddDismiss = { addMenu = false },
                                    onAddContact = { addMenu = false; sheet = Sheet.AddContact },
                                    onNewGroup = { addMenu = false; sheet = Sheet.NewGroup },
                                    onJoinGroup = { addMenu = false; sheet = Sheet.JoinGroup },
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
                                            draft = draft,
                                            onDraft = { draft = it },
                                            busy = busy,
                                            showBack = false,
                                            onBack = { selected = null },
                                            onSettings = { showSettings = true },
                                            chatMenu = chatMenu,
                                            onChatMenu = { chatMenu = it },
                                            onSend = { sendChat(c, chat, draft, messages, outgoing, runIo = { runIo(it) }) { draft = "" } },
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
                                                    applyIncoming(messages, listOf(r))
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
                                draft = draft,
                                onDraft = { draft = it },
                                busy = busy,
                                showBack = true,
                                onBack = { selected = null },
                                onSettings = { showSettings = true },
                                chatMenu = chatMenu,
                                onChatMenu = { chatMenu = it },
                                onSend = { sendChat(c, chat, draft, messages, outgoing, runIo = { runIo(it) }) { draft = "" } },
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
                                        applyIncoming(messages, listOf(r))
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
                                onAddContact = { addMenu = false; sheet = Sheet.AddContact },
                                onNewGroup = { addMenu = false; sheet = Sheet.NewGroup },
                                onJoinGroup = { addMenu = false; sheet = Sheet.JoinGroup },
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
    }
}

private fun sendChat(
    client: NemoClient?,
    chat: ChatTarget,
    draft: String,
    messages: MutableList<DisplayRow>,
    outgoing: MutableMap<String, Boolean>,
    runIo: (suspend () -> Unit) -> Unit,
    clear: () -> Unit,
) {
    val text = draft.trim()
    if (text.isEmpty()) return
    clear()
    runIo {
        val row = withContext(Dispatchers.IO) {
            if (chat.isGroup) client?.sendGroupText(chat.id, text)
            else client?.sendText(chat.id, text)
        } ?: return@runIo
        messages.add(row)
        outgoing["${row.convId}:${row.convSeq}:${row.kind}"] = true
    }
}

private fun attachFilePath(
    client: NemoClient?,
    chat: ChatTarget,
    path: String,
    messages: MutableList<DisplayRow>,
    outgoing: MutableMap<String, Boolean>,
    snackbar: SnackbarHostState,
    runIo: (suspend () -> Unit) -> Unit,
) {
    runIo {
        val f = File(path)
        if (!f.isFile) throw IllegalArgumentException("File not found")
        val row = withContext(Dispatchers.IO) {
            if (chat.isGroup) {
                client?.sendGroupFile(chat.id, f.name, "application/octet-stream", f.readBytes())
            } else {
                client?.sendFile(chat.id, f.name, "application/octet-stream", f.readBytes())
            }
        } ?: return@runIo
        messages.add(row)
        outgoing["${row.convId}:${row.convSeq}:${row.kind}"] = true
        snackbar.showSnackbar("Sent ${f.name}")
    }
}

@Composable
private fun OnboardScaffold(title: String, subtitle: String, content: @Composable () -> Unit) {
    Box(
        Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .statusBarsPadding()
            .navigationBarsPadding(),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier
                .widthIn(max = 420.dp)
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Box(
                Modifier
                    .size(72.dp)
                    .clip(CircleShape)
                    .background(MaterialTheme.colorScheme.primary),
                contentAlignment = Alignment.Center,
            ) {
                Icon(Icons.Filled.Lock, contentDescription = null, tint = MaterialTheme.colorScheme.onPrimary, modifier = Modifier.size(32.dp))
            }
            Spacer(Modifier.height(16.dp))
            Text("Nemo", style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.SemiBold)
            Text(title, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(8.dp))
            Text(subtitle, style = MaterialTheme.typography.bodyMedium, textAlign = TextAlign.Center, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(20.dp))
            content()
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
    Scaffold(
        modifier = modifier,
        topBar = {
            CenterAlignedTopAppBar(
                title = { Text(if (label == "Nemo") "Chats" else "$label · Chats", fontWeight = FontWeight.SemiBold) },
                colors = TopAppBarDefaults.centerAlignedTopAppBarColors(
                    containerColor = MaterialTheme.colorScheme.surface,
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
                FloatingActionButton(onClick = onAdd) {
                    Icon(Icons.Filled.Add, contentDescription = "New chat")
                }
                DropdownMenu(expanded = addMenu, onDismissRequest = onAddDismiss) {
                    DropdownMenuItem(
                        text = { Text("New chat") },
                        leadingIcon = { Icon(Icons.Filled.PersonAdd, null) },
                        onClick = onAddContact,
                    )
                    DropdownMenuItem(
                        text = { Text("New group") },
                        leadingIcon = { Icon(Icons.Filled.Group, null) },
                        onClick = onNewGroup,
                    )
                    DropdownMenuItem(
                        text = { Text("Join group") },
                        leadingIcon = { Icon(Icons.AutoMirrored.Filled.Chat, null) },
                        onClick = onJoinGroup,
                    )
                }
            }
        },
    ) { padding ->
        if (chats.isEmpty()) {
            Column(
                Modifier.fillMaxSize().padding(padding).padding(32.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                Icon(Icons.AutoMirrored.Filled.Chat, null, modifier = Modifier.size(48.dp), tint = MaterialTheme.colorScheme.outline)
                Spacer(Modifier.height(12.dp))
                Text("No conversations yet", style = MaterialTheme.typography.titleMedium)
                Text(
                    "Connect to your home server, then add someone with a Nemo contact card.",
                    style = MaterialTheme.typography.bodyMedium,
                    textAlign = TextAlign.Center,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
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
private fun ChatRow(
    chat: ChatTarget,
    preview: String,
    time: String,
    selected: Boolean,
    onClick: () -> Unit,
) {
    val bg = if (selected) MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.45f) else Color.Transparent
    Row(
        Modifier
            .fillMaxWidth()
            .background(bg)
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Avatar(chat.title, chat.isGroup)
        Spacer(Modifier.width(12.dp))
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
                    Text(time, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Text(
                preview,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
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
    draft: String,
    onDraft: (String) -> Unit,
    busy: Boolean,
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
) {
    val listState = rememberLazyListState()
    val dark = isSystemInDarkTheme()
    val wallpaper = if (dark) NemoChatDark else NemoChatLight
    LaunchedEffect(messages.size, chat.id) {
        if (messages.isNotEmpty()) listState.animateScrollToItem(messages.lastIndex)
    }
    Scaffold(
        containerColor = wallpaper,
        topBar = {
            TopAppBar(
                title = {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Avatar(chat.title, chat.isGroup, size = 36.dp)
                        Spacer(Modifier.width(10.dp))
                        Column {
                            Text(chat.title, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(
                                if (chat.isGroup) "Group" else "End-to-end encrypted",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                },
                navigationIcon = {
                    if (showBack) {
                        IconButton(onClick = onBack) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                        }
                    }
                },
                actions = {
                    if (!chat.isGroup) {
                        IconButton(onClick = onCall) { Icon(Icons.Filled.Call, contentDescription = "Call") }
                    }
                    IconButton(onClick = { onChatMenu(true) }) {
                        Icon(Icons.Filled.MoreVert, contentDescription = "More")
                    }
                    DropdownMenu(expanded = chatMenu, onDismissRequest = { onChatMenu(false) }) {
                        DropdownMenuItem(text = { Text("Answer call") }, onClick = { onChatMenu(false); onAnswer() })
                        DropdownMenuItem(text = { Text("Decline call") }, onClick = { onChatMenu(false); onDecline() })
                        DropdownMenuItem(
                            text = { Text("Hang up") },
                            leadingIcon = { Icon(Icons.Filled.CallEnd, null) },
                            onClick = { onChatMenu(false); onHangup() },
                        )
                        DropdownMenuItem(text = { Text("Chat settings") }, onClick = { onChatMenu(false); onSettings() })
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = MaterialTheme.colorScheme.surface),
            )
        },
        bottomBar = {
            Surface(tonalElevation = 2.dp) {
                Row(
                    Modifier.fillMaxWidth().padding(8.dp),
                    verticalAlignment = Alignment.Bottom,
                ) {
                    IconButton(onClick = onAttach) {
                        Icon(Icons.Filled.AttachFile, contentDescription = "Attach")
                    }
                    OutlinedTextField(
                        value = draft,
                        onValueChange = onDraft,
                        modifier = Modifier.weight(1f),
                        placeholder = { Text("Message") },
                        shape = RoundedCornerShape(24.dp),
                        maxLines = 5,
                    )
                    Spacer(Modifier.width(8.dp))
                    FilledIconButton(
                        onClick = onSend,
                        enabled = !busy && draft.isNotBlank(),
                    ) {
                        Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Send")
                    }
                }
            }
        },
    ) { padding ->
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize().padding(padding),
            contentPadding = PaddingValues(12.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            items(messages, key = { "${it.convId}:${it.convSeq}:${it.kind}:${it.target}" }) { row ->
                val mine = outgoing["${row.convId}:${row.convSeq}:${row.kind}"] == true
                MessageBubble(row, mine, onReact = { onReact(row) }, onDelete = { onDelete(row) })
            }
        }
    }
}

@Composable
private fun MessageBubble(
    row: DisplayRow,
    mine: Boolean,
    onReact: () -> Unit,
    onDelete: () -> Unit,
) {
    var menu by remember { mutableStateOf(false) }
    val dark = isSystemInDarkTheme()
    val bg = when {
        mine && dark -> NemoOutgoingDark
        mine -> NemoOutgoingLight
        dark -> NemoIncomingDark
        else -> Color.White
    }
    val shape = RoundedCornerShape(
        topStart = 16.dp,
        topEnd = 16.dp,
        bottomStart = if (mine) 16.dp else 4.dp,
        bottomEnd = if (mine) 4.dp else 16.dp,
    )
    Row(
        Modifier.fillMaxWidth(),
        horizontalArrangement = if (mine) Arrangement.End else Arrangement.Start,
    ) {
        Box {
            Column(
                Modifier
                    .widthIn(max = 320.dp)
                    .clip(shape)
                    .background(bg)
                    .clickable { menu = true }
                    .padding(horizontal = 12.dp, vertical = 8.dp),
            ) {
                Text(bubbleText(row), style = MaterialTheme.typography.bodyLarge)
                Text(
                    formatTime(row.sentAt),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.align(Alignment.End),
                )
            }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(text = { Text("React 👍") }, onClick = { menu = false; onReact() })
                DropdownMenuItem(text = { Text("Delete") }, onClick = { menu = false; onDelete() })
            }
        }
    }
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
                    "Envelope bytes never change. High sends dummy envelopes to a contact. Maximum keeps a ~2s slot and turns calls off. Tor is used for client→home when the home is not on loopback.",
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
private fun SheetForm(
    title: String,
    action: String,
    enabled: Boolean,
    onAction: () -> Unit,
    content: @Composable () -> Unit,
) {
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
private fun PassField(label: String, value: String, onChange: (String) -> Unit) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        visualTransformation = PasswordVisualTransformation(),
        modifier = Modifier.fillMaxWidth().testTag(label),
        singleLine = true,
        shape = RoundedCornerShape(12.dp),
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

internal fun applyIncoming(messages: MutableList<DisplayRow>, rows: List<DisplayRow>) {
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
