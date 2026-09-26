import org.gradle.api.tasks.testing.Test
import org.jetbrains.compose.desktop.application.dsl.TargetFormat
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.kotlin.multiplatform)
    alias(libs.plugins.compose.multiplatform)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.android.kotlin.multiplatform.library)
    alias(libs.plugins.ktlint)
    alias(libs.plugins.detekt)
}

group = "org.nemo"
version = "0.1.0"

val repoRoot = rootProject.projectDir.parentFile.parentFile
val ffiDebugDir = repoRoot.resolve("target/debug")
val ffiReleaseDir = repoRoot.resolve("target/release")
val ffiLibDir = ffiDebugDir // desktopTest + default JNA path
val jniLibsDir = project.layout.projectDirectory.dir("src/androidMain/jniLibs")

kotlin {
    jvmToolchain(21)
    android {
        namespace = "org.nemo.shared"
        // API 37 is published as platforms;android-37.0 (not android-37).
        compileSdk {
            version = release(37) {
                minorApiLevel = 0
            }
        }
        minSdk = 36
        androidResources {
            enable = true
        }
        withDeviceTest {
            instrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        }
        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_11)
        }
    }
    jvm("desktop") {
        compilerOptions {
            jvmTarget.set(JvmTarget.JVM_21)
        }
    }

    sourceSets {
        val commonMain by getting {
            dependencies {
                implementation(libs.compose.runtime)
                implementation(libs.compose.foundation)
                implementation(libs.compose.ui)
            }
        }
        val sharedJvm by creating {
            dependsOn(commonMain)
            dependencies {
                implementation(libs.compose.runtime)
                implementation(libs.compose.foundation)
                implementation(libs.compose.material3)
                implementation(libs.compose.ui)
                implementation(libs.compose.material.icons.extended)
                implementation(libs.kotlinx.coroutines.core)
                compileOnly(libs.jna)
                implementation(libs.zxing.core)
            }
            kotlin.srcDir("src/sharedJvm/kotlin")
        }
        val androidMain by getting {
            dependsOn(sharedJvm)
            dependencies {
                implementation("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")
                implementation(libs.androidx.activity.compose)
                implementation(libs.androidx.core.ktx)
                implementation(libs.stream.webrtc.android)
            }
        }
        val androidDeviceTest by getting {
            dependencies {
                implementation(kotlin("test"))
                implementation(libs.androidx.compose.ui.test.junit4)
                implementation(libs.androidx.test.ext.junit)
                implementation(libs.androidx.test.runner)
                implementation(libs.androidx.compose.ui.test.manifest)
            }
        }
        val desktopMain by getting {
            dependsOn(sharedJvm)
            dependencies {
                implementation(compose.desktop.currentOs)
                implementation(libs.kotlinx.coroutines.swing)
                implementation(libs.jna)
            }
        }
        val desktopTest by getting {
            dependencies {
                implementation(kotlin("test"))
                implementation(libs.jna)
            }
        }
    }
}

compose.desktop {
    application {
        mainClass = "org.nemo.MainKt"
        // JNA path is set per-task (debug vs release) in afterEvaluate.
        nativeDistributions {
            targetFormats(TargetFormat.Deb)
            packageName = "nemo-messenger"
            packageVersion = "0.1.0"
            description = "Nemo Messenger"
            vendor = "Nemo"
            copyright = "Copyright (c) 2026 Martin Mkhitaryan. AGPL-3.0-only."
            linux {
                debMaintainer = "nemo"
                menuGroup = "Network"
                iconFile.set(project.file("src/desktopMain/resources/nemo_logo_512.png"))
            }
        }
    }
}

ktlint {
    filter {
        exclude("**/uniffi/**")
        exclude("**/build/**")
    }
}

detekt {
    buildUponDefaultConfig = true
    parallel = true
    config.setFrom(files("$projectDir/config/detekt/detekt.yml"))
    source.setFrom(
        files(
            "src/commonMain/kotlin",
            "src/sharedJvm/kotlin",
            "src/androidMain/kotlin",
            "src/desktopMain/kotlin",
            "src/desktopTest/kotlin",
            "src/androidDeviceTest/kotlin",
        ),
    )
}

tasks.withType<io.gitlab.arturbosch.detekt.Detekt>().configureEach {
    exclude("**/uniffi/**")
    exclude("**/build/**")
}

tasks.register<Exec>("cargoBuildFfi") {
    group = "nemo"
    workingDir = repoRoot
    environment("CARGO_TARGET_DIR", repoRoot.resolve("target").absolutePath)
    commandLine("cargo", "build", "-p", "nemo-ffi", "-p", "nemo-server")
}

tasks.register<Exec>("cargoBuildFfiRelease") {
    group = "nemo"
    workingDir = repoRoot
    environment("CARGO_TARGET_DIR", repoRoot.resolve("target").absolutePath)
    commandLine("cargo", "build", "--release", "-p", "nemo-ffi", "-p", "nemo-server")
}

tasks.register<Exec>("cargoNdkFfi") {
    group = "nemo"
    workingDir = repoRoot
    environment("CARGO_TARGET_DIR", repoRoot.resolve("target").absolutePath)
    val ndk = System.getenv("ANDROID_NDK_HOME")
        ?: System.getenv("ANDROID_NDK_ROOT")
        ?: ""
    if (ndk.isNotEmpty()) {
        environment("ANDROID_NDK_HOME", ndk)
        environment("ANDROID_NDK_ROOT", ndk)
    }
    // Android 16 requires 16 KiB ELF segments; NDK r27 does not default them.
    environment(
        "CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS",
        "-C link-arg=-Wl,-z,max-page-size=16384",
    )
    environment(
        "CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS",
        "-C link-arg=-Wl,-z,max-page-size=16384",
    )
    commandLine(
        "cargo",
        "ndk",
        "-t",
        "arm64-v8a",
        "-t",
        "x86_64",
        "-P",
        "35",
        "-o",
        jniLibsDir.asFile.absolutePath,
        "build",
        "-p",
        "nemo-ffi",
        "--release",
    )
}

tasks.register<Exec>("generateUniffi") {
    group = "nemo"
    dependsOn("cargoBuildFfi")
    workingDir = repoRoot
    environment("CARGO_TARGET_DIR", repoRoot.resolve("target").absolutePath)
    commandLine(
        "cargo",
        "run",
        "-p",
        "nemo-ffi",
        "--features",
        "bindgen",
        "--bin",
        "uniffi-bindgen",
        "--",
        "generate",
        "--library",
        "target/debug/libnemo_ffi.so",
        "--language",
        "kotlin",
        "--out-dir",
        "apps/compose/composeApp/src/sharedJvm/kotlin",
        "--no-format",
    )
}

afterEvaluate {
    tasks.named<Test>("desktopTest") {
        dependsOn("cargoBuildFfi")
        systemProperty("jna.library.path", ffiDebugDir.absolutePath)
        environment("jna.library.path", ffiDebugDir.absolutePath)
    }
    fun org.gradle.api.tasks.JavaExec.withFfi(dir: java.io.File, cargoTask: String) {
        dependsOn(cargoTask)
        systemProperty("jna.library.path", dir.absolutePath)
        environment("jna.library.path", dir.absolutePath)
        jvmArgs = (jvmArgs ?: emptyList()).filterNot { it.startsWith("-Djna.library.path=") } +
            "-Djna.library.path=${dir.absolutePath}"
    }
    tasks.findByName("run")?.let { (it as org.gradle.api.tasks.JavaExec).withFfi(ffiDebugDir, "cargoBuildFfi") }
    tasks.findByName("runRelease")?.let {
        (it as org.gradle.api.tasks.JavaExec).withFfi(ffiReleaseDir, "cargoBuildFfiRelease")
    }
    // Desktop installers only. Android packaging must not build host nemo-ffi
    // (that pulls webrtc-audio-processing and needs Meson).
    tasks.matching {
        it.name.startsWith("packageDeb") ||
            it.name.startsWith("packageMsi") ||
            it.name.startsWith("packageDmg") ||
            it.name.startsWith("packageExe") ||
            it.name.startsWith("packageUber") ||
            it.name.startsWith("packageDistribution") ||
            it.name.startsWith("packageRelease") ||
            it.name.startsWith("createReleaseDistributable") ||
            it.name == "runReleaseDistributable" ||
            it.name == "runDistributable"
    }.configureEach { dependsOn("cargoBuildFfiRelease") }
    tasks.matching { it.name.contains("AndroidTest") && it.name.startsWith("connected") }.configureEach {
        dependsOn("cargoNdkFfi")
    }
    tasks.matching { it.name.startsWith("connected") && it.name.contains("DeviceTest") }.configureEach {
        dependsOn("cargoNdkFfi")
    }
}
