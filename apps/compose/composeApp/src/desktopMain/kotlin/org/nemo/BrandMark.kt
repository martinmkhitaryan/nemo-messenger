package org.nemo

import androidx.compose.foundation.Image
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.decodeToImageBitmap
import androidx.compose.ui.graphics.painter.BitmapPainter
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.layout.ContentScale
import java.io.InputStream

/** Painter for the PNGs under desktopMain/resources. The string-based
 * `painterResource` and `loadImageBitmap` are deprecated, so decode straight
 * from the classpath (no generated code, no new dependencies). */
@Composable
fun classpathPainter(path: String): Painter {
    val bitmap = remember(path) {
        val stream: InputStream = requireNotNull(
            object {}.javaClass.classLoader?.getResourceAsStream(path),
        ) { "missing desktop resource: $path" }
        stream.use { it.readBytes().decodeToImageBitmap() }
    }
    return remember(bitmap) { BitmapPainter(bitmap) }
}

@Composable
actual fun NemoBrandMark(modifier: Modifier) {
    Image(
        painter = classpathPainter("nemo_brand_round.png"),
        contentDescription = "Nemo",
        modifier = modifier.clip(CircleShape),
        contentScale = ContentScale.Crop,
    )
}
