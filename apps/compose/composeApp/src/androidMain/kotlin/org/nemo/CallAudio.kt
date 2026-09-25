package org.nemo

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.AudioTrack
import android.media.MediaRecorder
import android.media.audiofx.AcousticEchoCanceler
import android.media.audiofx.AutomaticGainControl
import android.media.audiofx.NoiseSuppressor
import android.os.Process
import org.webrtc.audio.JavaAudioDeviceModule
import uniffi.nemo.NemoClient
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.atomic.AtomicBoolean

internal object CallAudio {
    private const val SAMPLE_RATE = 48_000
    private val running = AtomicBoolean(false)

    @Volatile private var record: AudioRecord? = null

    @Volatile private var captureThread: Thread? = null

    @Volatile private var playThread: Thread? = null

    @Volatile private var effects: List<AutoCloseable> = emptyList()

    @Volatile private var savedMode: Int = AudioManager.MODE_NORMAL

    fun start(context: Context, client: NemoClient) {
        if (!running.compareAndSet(false, true)) return
        val app = context.applicationContext
        val audio = app.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        savedMode = audio.mode
        audio.mode = AudioManager.MODE_IN_COMMUNICATION
        val minRec = AudioRecord.getMinBufferSize(
            SAMPLE_RATE,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_16BIT,
        ).coerceAtLeast(SAMPLE_RATE / 25 * 2)
        val rec = AudioRecord.Builder()
            .setAudioSource(MediaRecorder.AudioSource.VOICE_COMMUNICATION)
            .setAudioFormat(
                AudioFormat.Builder()
                    .setSampleRate(SAMPLE_RATE)
                    .setChannelMask(AudioFormat.CHANNEL_IN_MONO)
                    .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                    .build(),
            )
            .setBufferSizeInBytes(minRec)
            .build()
        val fx = mutableListOf<AutoCloseable>()
        val session = rec.audioSessionId
        if (JavaAudioDeviceModule.isBuiltInAcousticEchoCancelerSupported() &&
            AcousticEchoCanceler.isAvailable()
        ) {
            AcousticEchoCanceler.create(session)?.let {
                it.enabled = true
                fx += AutoCloseable { it.release() }
            }
        }
        if (JavaAudioDeviceModule.isBuiltInNoiseSuppressorSupported() &&
            NoiseSuppressor.isAvailable()
        ) {
            NoiseSuppressor.create(session)?.let {
                it.enabled = true
                fx += AutoCloseable { it.release() }
            }
        }
        if (AutomaticGainControl.isAvailable()) {
            AutomaticGainControl.create(session)?.let {
                it.enabled = true
                fx += AutoCloseable { it.release() }
            }
        }
        effects = fx
        rec.startRecording()
        record = rec
        captureThread = Thread {
            Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO)
            val buf = ByteArray(960 * 2)
            while (running.get()) {
                val n = rec.read(buf, 0, buf.size)
                if (n <= 0) continue
                val pcm = bytesToShorts(buf.copyOf(n))
                try {
                    client.pushCapturePcm(pcm.toList(), SAMPLE_RATE.toUInt(), 1u)
                } catch (_: Throwable) {
                    break
                }
            }
        }.also {
            it.name = "nemo-capture"
            it.start()
        }
        val minPlay = AudioTrack.getMinBufferSize(
            SAMPLE_RATE,
            AudioFormat.CHANNEL_OUT_MONO,
            AudioFormat.ENCODING_PCM_16BIT,
        ).coerceAtLeast(SAMPLE_RATE / 25 * 2)
        val track = AudioTrack.Builder()
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_VOICE_COMMUNICATION)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                    .build(),
            )
            .setAudioFormat(
                AudioFormat.Builder()
                    .setSampleRate(SAMPLE_RATE)
                    .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                    .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                    .build(),
            )
            .setBufferSizeInBytes(minPlay)
            .setTransferMode(AudioTrack.MODE_STREAM)
            .build()
        track.play()
        playThread = Thread {
            Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO)
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
                val shorts = ShortArray(pcm.size) { i -> pcm[i] }
                track.write(shorts, 0, shorts.size)
            }
            runCatching {
                track.stop()
                track.release()
            }
        }.also {
            it.name = "nemo-playback"
            it.start()
        }
    }

    fun stop() {
        if (!running.compareAndSet(true, false)) return
        captureThread?.interrupt()
        playThread?.interrupt()
        captureThread = null
        playThread = null
        runCatching {
            record?.stop()
            record?.release()
        }
        record = null
        effects.forEach { runCatching { it.close() } }
        effects = emptyList()
        appContext?.let { ctx ->
            val audio = ctx.getSystemService(Context.AUDIO_SERVICE) as AudioManager
            audio.mode = savedMode
        }
    }
}

internal fun bytesToShorts(data: ByteArray): ShortArray {
    val shorts = ShortArray(data.size / 2)
    ByteBuffer.wrap(data).order(ByteOrder.LITTLE_ENDIAN).asShortBuffer().get(shorts)
    return shorts
}
