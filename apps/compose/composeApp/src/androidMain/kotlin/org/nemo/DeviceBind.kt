package org.nemo

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

private const val ANDROID_KEYSTORE = "AndroidKeyStore"
private const val ALIAS = "org.nemo.vault.device"
private const val WRAP_FILE = "nemo-device.wrap"
private const val GCM_TAG_BITS = 128
private const val NONCE_LEN = 12

actual fun deviceVaultSecret(vaultPath: String): ByteArray {
    val vaultDir = File(vaultPath)
    val ctx = appContext
    val wrapDir = ctx?.noBackupFilesDir ?: vaultDir.parentFile ?: vaultDir
    return loadOrCreateDeviceSecret(File(wrapDir, WRAP_FILE))
}

internal fun loadOrCreateDeviceSecret(wrapFile: File): ByteArray {
    val ks = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
    val key = getOrCreateAesKey(ks)
    if (wrapFile.isFile) {
        val blob = wrapFile.readBytes()
        require(blob.size > NONCE_LEN) { "corrupt device wrap" }
        val nonce = blob.copyOfRange(0, NONCE_LEN)
        val ct = blob.copyOfRange(NONCE_LEN, blob.size)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(GCM_TAG_BITS, nonce))
        return cipher.doFinal(ct)
    }
    val secret = ByteArray(32).also { SecureRandom().nextBytes(it) }
    val cipher = Cipher.getInstance("AES/GCM/NoPadding")
    cipher.init(Cipher.ENCRYPT_MODE, key)
    val nonce = cipher.iv
    val ct = cipher.doFinal(secret)
    wrapFile.parentFile?.mkdirs()
    wrapFile.writeBytes(nonce + ct)
    return secret
}

private fun getOrCreateAesKey(ks: KeyStore): SecretKey {
    if (ks.containsAlias(ALIAS)) {
        return (ks.getEntry(ALIAS, null) as KeyStore.SecretKeyEntry).secretKey
    }
    val spec = KeyGenParameterSpec.Builder(
        ALIAS,
        KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
    )
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .setKeySize(256)
        .setRandomizedEncryptionRequired(true)
        .build()
    val kg = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
    kg.init(spec)
    return kg.generateKey()
}
