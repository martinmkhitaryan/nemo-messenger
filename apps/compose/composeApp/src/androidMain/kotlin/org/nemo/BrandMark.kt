package org.nemo

import androidx.compose.foundation.Image
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import org.nemo.shared.R

@Composable
actual fun NemoBrandMark(modifier: Modifier) {
    Image(
        painter = painterResource(R.drawable.nemo_brand_round),
        contentDescription = "Nemo",
        modifier = modifier.clip(CircleShape),
        contentScale = ContentScale.Crop,
    )
}
