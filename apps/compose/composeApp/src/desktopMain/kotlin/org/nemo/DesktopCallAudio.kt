package org.nemo

import uniffi.nemo.NemoClient
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.atomic.AtomicBoolean
import javax.sound.sampled.AudioFormat
import javax.sound.sampled.AudioSystem
import javax.sound.sampled.DataLine
import javax.sound.sampled.SourceDataLine
import javax.sound.sampled.TargetDataLine

internal object DesktopCallAudio {
    private const val SAMPLE_RATE = 48_000
    private val running = AtomicBoolean(false)
    private var captureThread: Thread? = null
    private var playThread: Thread? = null

    fun start(client: NemoClient) {
        if (!running.compareAndSet(false, true)) return
        val format = AudioFormat(SAMPLE_RATE.toFloat(), 16, 1, true, false)
        val rec = tryOpen<TargetDataLine>(format)
        val play = tryOpen<SourceDataLine>(format)
        if (rec == null && play == null) {
            running.set(false)
            return
        }
        rec?.start()
        play?.start()
        captureThread = rec?.let { line ->
            Thread {
                val buf = ByteArray(960 * 2)
                while (running.get()) {
                    val n = line.read(buf, 0, buf.size)
                    if (n <= 0) continue
                    val shorts = bytesToShorts(buf.copyOf(n))
                    try {
                        client.pushCapturePcm(shorts.toList(), SAMPLE_RATE.toUInt(), 1u)
                    } catch (_: Throwable) {
                        break
                    }
                }
                runCatching {
                    line.stop()
                    line.close()
                }
            }.also {
                it.name = "nemo-capture"
                it.isDaemon = true
                it.start()
            }
        }
        playThread = play?.let { line ->
            Thread {
                while (running.get()) {
                    val pcm = try {
                        client.pullPlaybackPcm(960u)
                    } catch (_: Throwable) {
                        break
                    }
                    if (pcm.isEmpty()) {
                        try {
                            Thread.sleep(5)
                        } catch (_: InterruptedException) {
                            break
                        }
                        continue
                    }
                    val bytes = shortsToBytes(ShortArray(pcm.size) { i -> pcm[i] })
                    line.write(bytes, 0, bytes.size)
                }
                runCatching {
                    line.stop()
                    line.close()
                }
            }.also {
                it.name = "nemo-playback"
                it.isDaemon = true
                it.start()
            }
        }
    }

    fun stop() {
        if (!running.compareAndSet(true, false)) return
        captureThread?.interrupt()
        playThread?.interrupt()
        captureThread = null
        playThread = null
    }

    private inline fun <reified T : DataLine> tryOpen(format: AudioFormat): T? {
        return try {
            val info = DataLine.Info(T::class.java, format)
            if (!AudioSystem.isLineSupported(info)) return null
            @Suppress("UNCHECKED_CAST")
            (AudioSystem.getLine(info) as T).also { line ->
                when (line) {
                    is TargetDataLine -> line.open(format)
                    is SourceDataLine -> line.open(format)
                    else -> line.open()
                }
            }
        } catch (_: Throwable) {
            null
        }
    }

    private fun bytesToShorts(data: ByteArray): ShortArray {
        val shorts = ShortArray(data.size / 2)
        ByteBuffer.wrap(data).order(ByteOrder.LITTLE_ENDIAN).asShortBuffer().get(shorts)
        return shorts
    }

    private fun shortsToBytes(samples: ShortArray): ByteArray {
        val buf = ByteBuffer.allocate(samples.size * 2).order(ByteOrder.LITTLE_ENDIAN)
        buf.asShortBuffer().put(samples)
        return buf.array()
    }
}
