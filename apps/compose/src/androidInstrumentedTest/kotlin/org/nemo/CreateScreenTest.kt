package org.nemo

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import kotlin.test.assertTrue

@RunWith(AndroidJUnit4::class)
class CreateScreenTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun createScreenStatesUnrecoverableAndRejectsShortPassphrase() {
        val dir = File(
            InstrumentationRegistry.getInstrumentation().targetContext.cacheDir,
            "ui-${System.nanoTime()}",
        )
        dir.mkdirs()
        compose.setContent {
            NemoTheme(mode = NemoThemeMode.System, onModeChange = {}) {
                SessionPane(label = "Nemo", vaultDir = dir, modifier = Modifier.fillMaxSize())
            }
        }
        compose.onAllNodesWithText("Create identity")[0].assertIsDisplayed()
        compose.onNodeWithText("cannot be recovered or exported", substring = true).assertIsDisplayed()
        compose.onNodeWithTag("Passphrase (min 8)").performTextInput("short")
        compose.onNodeWithTag("Confirm").performTextInput("short")
        compose.onNodeWithTag("create-identity").performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Passphrase must be at least 8 characters")
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        assertTrue(dir.list().isNullOrEmpty() || !File(dir, "kdf.cbor").isFile)
    }
}
