import org.gradle.api.tasks.testing.Test
import org.jetbrains.compose.desktop.application.dsl.TargetFormat
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    kotlin("multiplatform") version "2.0.21"
    id("org.jetbrains.compose") version "1.7.1"
    id("org.jetbrains.kotlin.plugin.compose") version "2.0.21"
    id("com.android.application") version "8.9.1"
}

group = "org.nemo"
version = "0.1.0"

val repoRoot = rootDir.parentFile.parentFile
val ffiLibDir = repoRoot.resolve("target/debug")
val jniLibsDir = project.layout.projectDirectory.dir("src/androidMain/jniLibs")

kotlin {
    jvmToolchain(21)
    androidTarget {
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
        val commonMain by getting
        val sharedJvm by creating {
            dependsOn(commonMain)
            dependencies {
                implementation(compose.runtime)
                implementation(compose.foundation)
                implementation(compose.material3)
                implementation(compose.ui)
                implementation(compose.materialIconsExtended)
                implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.9.0")
                compileOnly("net.java.dev.jna:jna:5.15.0")
                implementation("com.google.zxing:core:3.5.3")
            }
            kotlin.srcDir("src/sharedJvm/kotlin")
        }
        val androidMain by getting {
            dependsOn(sharedJvm)
            dependencies {
                implementation("net.java.dev.jna:jna:5.15.0@aar")
                implementation("androidx.activity:activity-compose:1.9.3")
                implementation("androidx.core:core-ktx:1.15.0")
                implementation("io.getstream:stream-webrtc-android:1.3.8")
            }
        }
        val androidInstrumentedTest by getting {
            dependencies {
                implementation(kotlin("test"))
                implementation("androidx.compose.ui:ui-test-junit4:1.7.8")
                implementation("androidx.test.ext:junit:1.2.1")
                implementation("androidx.test:runner:1.6.2")
            }
        }
        val desktopMain by getting {
            dependsOn(sharedJvm)
            dependencies {
                implementation(compose.desktop.currentOs)
                implementation("org.jetbrains.kotlinx:kotlinx-coroutines-swing:1.9.0")
                implementation("net.java.dev.jna:jna:5.15.0")
            }
        }
        val desktopTest by getting {
            dependencies {
                implementation(kotlin("test"))
                implementation("net.java.dev.jna:jna:5.15.0")
            }
        }
    }
}

android {
    namespace = "org.nemo"
    compileSdk = 36
    ndkVersion = "27.2.12479018"
    defaultConfig {
        applicationId = "org.nemo"
        minSdk = 36
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    packaging {
        jniLibs {
            keepDebugSymbols += setOf("**/libnemo_ffi.so")
        }
    }
}

dependencies {
    debugImplementation("androidx.compose.ui:ui-test-manifest:1.7.8")
}

compose.desktop {
    application {
        mainClass = "org.nemo.MainKt"
        jvmArgs += "-Djna.library.path=${ffiLibDir.absolutePath}"
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
            }
        }
    }
}

tasks.register<Exec>("cargoBuildFfi") {
    group = "nemo"
    workingDir = repoRoot
    environment("CARGO_TARGET_DIR", repoRoot.resolve("target").absolutePath)
    commandLine("cargo", "build", "-p", "nemo-ffi", "-p", "nemo-server")
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
        "apps/compose/src/sharedJvm/kotlin",
        "--no-format",
    )
}

afterEvaluate {
    tasks.named<Test>("desktopTest") {
        dependsOn("cargoBuildFfi")
        systemProperty("jna.library.path", ffiLibDir.absolutePath)
        environment("jna.library.path", ffiLibDir.absolutePath)
    }
    tasks.findByName("run")?.dependsOn("cargoBuildFfi")
    // Desktop installers only. Android `packageDebug` / `packageRelease` must not
    // build host `nemo-ffi` (that pulls webrtc-audio-processing and needs Meson).
    tasks.matching {
        it.name.startsWith("packageDeb") ||
            it.name.startsWith("packageMsi") ||
            it.name.startsWith("packageDmg") ||
            it.name.startsWith("packageExe") ||
            it.name.startsWith("packageUber") ||
            it.name.startsWith("packageDistribution")
    }.configureEach { dependsOn("cargoBuildFfi") }
    tasks.findByName("preBuild")?.dependsOn("cargoNdkFfi")
    tasks.matching { it.name.contains("AndroidTest") && it.name.startsWith("connected") }.configureEach {
        dependsOn("cargoNdkFfi")
    }
}
