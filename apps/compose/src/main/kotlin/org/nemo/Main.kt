package org.nemo

import androidx.compose.foundation.VerticalScrollbar
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.rememberScrollbarAdapter
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.nemo.DisplayRow
import uniffi.nemo.NemoClient
import java.io.File

private const val DEFAULT_HOME = "http://127.0.0.1:8787"
private const val CANNOT_RECOVER =
    "This identity cannot be recovered. If you lose the passphrase, the keys are gone. Write the revocation phrase down; it is shown once."

fun main() {
    val root = repoRoot()
    val libDir = File(root, "target/debug")
    if (libDir.resolve("libnemo_ffi.so").isFile) {
        System.setProperty("jna.library.path", libDir.absolutePath)
    }
    val data = File(System.getProperty("user.home"), ".local/share/nemo")
    application {
        Window(onCloseRequest = ::exitApplication, title = "Nemo") {
            MaterialTheme {
                Row(Modifier.fillMaxSize()) {
                    SessionPane(
                        label = "Left",
                        vaultDir = File(data, "left"),
                        modifier = Modifier.weight(1f).fillMaxHeight(),
                    )
                    VerticalDivider()
                    SessionPane(
                        label = "Right",
                        vaultDir = File(data, "right"),
                        modifier = Modifier.weight(1f).fillMaxHeight(),
                    )
                }
            }
        }
    }
}

private fun repoRoot(): File {
    var dir = File(System.getProperty("user.dir")).absoluteFile
    repeat(8) {
        if (File(dir, "crates/nemo-ffi").isDirectory) return dir
        dir = dir.parentFile ?: return File(".")
    }
    return File(".")
}

private enum class Phase { Locked, Create, Mnemonic, Home }

@Composable
private fun SessionPane(label: String, vaultDir: File, modifier: Modifier = Modifier) {
    val scope = rememberCoroutineScope()
    var phase by remember {
        mutableStateOf(if (File(vaultDir, "kdf.cbor").isFile) Phase.Locked else Phase.Create)
    }
    var client by remember { mutableStateOf<NemoClient?>(null) }
    var passphrase by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf("") }
    var mnemonic by remember { mutableStateOf<String?>(null) }
    var fingerprint by remember { mutableStateOf("") }
    var identityHex by remember { mutableStateOf("") }
    var homeUrl by remember { mutableStateOf(DEFAULT_HOME) }
    var shareUri by remember { mutableStateOf("") }
    var cardPaste by remember { mutableStateOf("") }
    var nickname by remember { mutableStateOf(label) }
    var peerId by remember { mutableStateOf("") }
    var draft by remember { mutableStateOf("") }
    var groupName by remember { mutableStateOf(label) }
    var groupId by remember { mutableStateOf("") }
    var inviteUri by remember { mutableStateOf("") }
    var invitePaste by remember { mutableStateOf("") }
    var joinUri by remember { mutableStateOf("") }
    var joinPaste by remember { mutableStateOf("") }
    var groupDraft by remember { mutableStateOf("") }
    var memberCred by remember { mutableStateOf("") }
    var filePath by remember { mutableStateOf("") }
    var status by remember { mutableStateOf<String?>(null) }
    val messages = remember { mutableStateListOf<DisplayRow>() }
    var busy by remember { mutableStateOf(false) }

    fun runIo(block: suspend () -> Unit) {
        if (busy) return
        busy = true
        status = null
        scope.launch {
            try {
                block()
            } catch (e: Throwable) {
                status = e.message ?: e.toString()
            } finally {
                busy = false
            }
        }
    }

    LaunchedEffect(client, phase) {
        val c = client ?: return@LaunchedEffect
        if (phase != Phase.Home) return@LaunchedEffect
        while (true) {
            delay(2_000)
            try {
                val rows = withContext(Dispatchers.IO) { c.fetchNow() }
                if (rows.isNotEmpty()) {
                    messages.addAll(rows)
                }
                if (groupId.isEmpty()) {
                    val groups = withContext(Dispatchers.IO) { c.listGroups() }
                    if (groups.isNotEmpty()) {
                        groupId = groups[0].groupId
                    }
                }
            } catch (_: Throwable) {
            }
        }
    }

    Surface(modifier) {
        Column(Modifier.fillMaxSize().padding(16.dp)) {
            Text("$label · ${vaultDir.name}", style = MaterialTheme.typography.titleMedium)
            status?.let {
                Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
            }
            Spacer(Modifier.height(8.dp))
            when (phase) {
                Phase.Create -> {
                    Text(CANNOT_RECOVER, style = MaterialTheme.typography.bodySmall)
                    Spacer(Modifier.height(8.dp))
                    PassField("Passphrase (min 8)", passphrase) { passphrase = it }
                    PassField("Confirm", confirm) { confirm = it }
                    Button(
                        enabled = !busy,
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
                                    NemoClient.createAt(vaultDir.absolutePath, passphrase)
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
                Phase.Locked -> {
                    Text("Unlock this vault. There is no recovery if the passphrase is wrong.")
                    PassField("Passphrase", passphrase) { passphrase = it }
                    Button(
                        enabled = !busy,
                        onClick = {
                            runIo {
                                val c = withContext(Dispatchers.IO) {
                                    NemoClient.openAt(vaultDir.absolutePath, passphrase)
                                }
                                client = c
                                fingerprint = withContext(Dispatchers.IO) { c.fingerprint() }
                                identityHex = withContext(Dispatchers.IO) { c.identityIdHex() }
                                val groups = withContext(Dispatchers.IO) { c.listGroups() }
                                if (groups.isNotEmpty()) {
                                    groupId = groups[0].groupId
                                }
                                phase = Phase.Home
                            }
                        },
                    ) { Text("Unlock") }
                    TextButton(onClick = { phase = Phase.Create }) { Text("Create a new identity") }
                }
                Phase.Mnemonic -> {
                    Text("Write this revocation phrase down. It is never stored.")
                    Spacer(Modifier.height(8.dp))
                    Text(mnemonic ?: "", fontFamily = FontFamily.Monospace)
                    Spacer(Modifier.height(8.dp))
                    Button(
                        onClick = {
                            runIo {
                                val c = client ?: return@runIo
                                fingerprint = withContext(Dispatchers.IO) { c.fingerprint() }
                                identityHex = withContext(Dispatchers.IO) { c.identityIdHex() }
                                phase = Phase.Home
                            }
                        },
                    ) { Text("I wrote it down") }
                }
                Phase.Home -> {
                    val scroll = rememberScrollState()
                    Box(Modifier.weight(1f)) {
                        Column(Modifier.verticalScroll(scroll).padding(end = 12.dp)) {
                            Text("Fingerprint", style = MaterialTheme.typography.labelSmall)
                            Text(fingerprint, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                            Text("identity_id", style = MaterialTheme.typography.labelSmall)
                            Text(identityHex, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                            OutlinedTextField(
                                value = homeUrl,
                                onValueChange = { homeUrl = it },
                                label = { Text("Home server") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                            )
                            Button(
                                enabled = !busy,
                                onClick = {
                                    runIo {
                                        withContext(Dispatchers.IO) { client?.register(homeUrl.trim()) }
                                        status = "Registered on ${homeUrl.trim()}"
                                    }
                                },
                            ) { Text("Register") }
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Button(
                                    enabled = !busy,
                                    onClick = {
                                        runIo {
                                            shareUri = withContext(Dispatchers.IO) {
                                                client?.mintShareUri().orEmpty()
                                            }
                                        }
                                    },
                                ) { Text("Mint share card") }
                            }
                            if (shareUri.isNotEmpty()) {
                                Text(shareUri, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                            }
                            OutlinedTextField(
                                value = cardPaste,
                                onValueChange = { cardPaste = it },
                                label = { Text("Paste contact card (nemo:1:…)") },
                                modifier = Modifier.fillMaxWidth(),
                            )
                            OutlinedTextField(
                                value = nickname,
                                onValueChange = { nickname = it },
                                label = { Text("Local nickname") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                            )
                            Button(
                                enabled = !busy,
                                onClick = {
                                    runIo {
                                        peerId = withContext(Dispatchers.IO) {
                                            client?.addContact(cardPaste.trim(), nickname).orEmpty()
                                        }
                                        status = "Added $peerId"
                                    }
                                },
                            ) { Text("Add contact") }
                            if (peerId.isNotEmpty()) {
                                Text("Peer $peerId", fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                            }
                            HorizontalDivider(Modifier.padding(vertical = 8.dp))
                            Text("Group", style = MaterialTheme.typography.titleSmall)
                            OutlinedTextField(
                                value = groupName,
                                onValueChange = { groupName = it },
                                label = { Text("Group nickname") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                            )
                            Button(
                                enabled = !busy,
                                onClick = {
                                    runIo {
                                        groupId = withContext(Dispatchers.IO) {
                                            client?.createGroup(groupName.ifBlank { label }).orEmpty()
                                        }
                                        status = "Created group $groupId"
                                    }
                                },
                            ) { Text("Create group") }
                            if (groupId.isNotEmpty()) {
                                Text("Group $groupId", fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                            }
                            Button(
                                enabled = !busy && groupId.isNotEmpty(),
                                onClick = {
                                    runIo {
                                        inviteUri = withContext(Dispatchers.IO) {
                                            client?.mintGroupInvite(groupId).orEmpty()
                                        }
                                    }
                                },
                            ) { Text("Mint group invite") }
                            if (inviteUri.isNotEmpty()) {
                                Text(inviteUri, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                            }
                            OutlinedTextField(
                                value = invitePaste,
                                onValueChange = { invitePaste = it },
                                label = { Text("Paste group invite (nemo-g:1:…)") },
                                modifier = Modifier.fillMaxWidth(),
                            )
                            Button(
                                enabled = !busy && invitePaste.isNotBlank(),
                                onClick = {
                                    runIo {
                                        joinUri = withContext(Dispatchers.IO) {
                                            client?.acceptGroupInvite(invitePaste.trim()).orEmpty()
                                        }
                                        status = "Accepted invite; give this join request to an existing member"
                                    }
                                },
                            ) { Text("Accept invite") }
                            if (joinUri.isNotEmpty()) {
                                Text(joinUri, fontFamily = FontFamily.Monospace, style = MaterialTheme.typography.bodySmall)
                            }
                            OutlinedTextField(
                                value = joinPaste,
                                onValueChange = { joinPaste = it },
                                label = { Text("Paste join request (nemo-j:1:…)") },
                                modifier = Modifier.fillMaxWidth(),
                            )
                            Button(
                                enabled = !busy && joinPaste.isNotBlank(),
                                onClick = {
                                    runIo {
                                        memberCred = withContext(Dispatchers.IO) {
                                            client?.admitJoin(joinPaste.trim()).orEmpty()
                                        }
                                        status = "Admitted $memberCred"
                                    }
                                },
                            ) { Text("Admit join") }
                            OutlinedTextField(
                                value = groupDraft,
                                onValueChange = { groupDraft = it },
                                label = { Text("Group message") },
                                modifier = Modifier.fillMaxWidth(),
                            )
                            Row {
                                Button(
                                    enabled = !busy && groupId.isNotEmpty() && groupDraft.isNotBlank(),
                                    onClick = {
                                        val text = groupDraft
                                        groupDraft = ""
                                        runIo {
                                            val row = withContext(Dispatchers.IO) {
                                                client?.sendGroupText(groupId, text)
                                            } ?: return@runIo
                                            messages.add(row)
                                        }
                                    },
                                ) { Text("Send group") }
                                Spacer(Modifier.width(8.dp))
                                Button(
                                    enabled = !busy && groupId.isNotEmpty() && memberCred.isNotEmpty(),
                                    onClick = {
                                        runIo {
                                            withContext(Dispatchers.IO) {
                                                client?.removeGroupMember(groupId, memberCred)
                                            }
                                            status = "Removed $memberCred"
                                        }
                                    },
                                ) { Text("Remove member") }
                            }
                            HorizontalDivider(Modifier.padding(vertical = 8.dp))
                            Text("Thread", style = MaterialTheme.typography.titleSmall)
                            messages.forEach { row ->
                                val who = when {
                                    peerId.isNotEmpty() && row.convId == peerId -> "You"
                                    groupId.isNotEmpty() && row.convId == groupId -> "Group"
                                    else -> "Them"
                                }
                                if (row.fileName.isNotEmpty()) {
                                    Text(
                                        "$who file: ${row.fileName} (${row.fileBytes.size} bytes)",
                                        style = MaterialTheme.typography.bodyMedium,
                                    )
                                } else {
                                    Text(
                                        "$who: ${row.text}",
                                        style = MaterialTheme.typography.bodyMedium,
                                    )
                                }
                            }
                            OutlinedTextField(
                                value = draft,
                                onValueChange = { draft = it },
                                label = { Text("Message") },
                                modifier = Modifier.fillMaxWidth(),
                            )
                            Row {
                                Button(
                                    enabled = !busy && peerId.isNotEmpty() && draft.isNotBlank(),
                                    onClick = {
                                        val text = draft
                                        draft = ""
                                        runIo {
                                            val row = withContext(Dispatchers.IO) {
                                                client?.sendText(peerId, text)
                                            } ?: return@runIo
                                            messages.add(row)
                                        }
                                    },
                                ) { Text("Send") }
                                Spacer(Modifier.width(8.dp))
                                Button(
                                    enabled = !busy,
                                    onClick = {
                                        runIo {
                                            val rows = withContext(Dispatchers.IO) {
                                                client?.fetchNow().orEmpty()
                                            }
                                            messages.addAll(rows)
                                        }
                                    },
                                ) { Text("Fetch") }
                            }
                            OutlinedTextField(
                                value = filePath,
                                onValueChange = { filePath = it },
                                label = { Text("File path") },
                                modifier = Modifier.fillMaxWidth(),
                                singleLine = true,
                            )
                            Row {
                                Button(
                                    enabled = !busy,
                                    onClick = {
                                        val dlg = java.awt.FileDialog(null as java.awt.Frame?, "Send file", java.awt.FileDialog.LOAD)
                                        dlg.isVisible = true
                                        val dir = dlg.directory
                                        val name = dlg.file
                                        if (!dir.isNullOrEmpty() && !name.isNullOrEmpty()) {
                                            filePath = java.io.File(dir, name).absolutePath
                                        }
                                    },
                                ) { Text("Browse") }
                                Spacer(Modifier.width(8.dp))
                                Button(
                                    enabled = !busy && peerId.isNotEmpty() && filePath.isNotBlank(),
                                    onClick = {
                                        runIo {
                                            val f = File(filePath.trim())
                                            if (!f.isFile) throw IllegalArgumentException("File not found")
                                            val row = withContext(Dispatchers.IO) {
                                                client?.sendFile(
                                                    peerId,
                                                    f.name,
                                                    "application/octet-stream",
                                                    f.readBytes(),
                                                )
                                            } ?: return@runIo
                                            messages.add(row)
                                            status = "Sent ${f.name}"
                                        }
                                    },
                                ) { Text("Send file") }
                                Spacer(Modifier.width(8.dp))
                                Button(
                                    enabled = !busy && groupId.isNotEmpty() && filePath.isNotBlank(),
                                    onClick = {
                                        runIo {
                                            val f = File(filePath.trim())
                                            if (!f.isFile) throw IllegalArgumentException("File not found")
                                            val row = withContext(Dispatchers.IO) {
                                                client?.sendGroupFile(
                                                    groupId,
                                                    f.name,
                                                    "application/octet-stream",
                                                    f.readBytes(),
                                                )
                                            } ?: return@runIo
                                            messages.add(row)
                                            status = "Sent group file ${f.name}"
                                        }
                                    },
                                ) { Text("Send group file") }
                            }
                        }
                        VerticalScrollbar(
                            adapter = rememberScrollbarAdapter(scroll),
                            modifier = Modifier.align(Alignment.CenterEnd).fillMaxHeight(),
                        )
                    }
                }
            }
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
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
    )
}
