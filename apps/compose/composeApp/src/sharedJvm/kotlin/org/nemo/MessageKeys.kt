package org.nemo

import uniffi.nemo.DisplayRow

/** Stable list identity for a vault row (optimistic locals keep their local id). */
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
