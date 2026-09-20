plugins {
    kotlin("jvm") version "2.0.21"
    id("org.jetbrains.compose") version "1.7.1"
    id("org.jetbrains.kotlin.plugin.compose") version "2.0.21"
}

group = "org.nemo"
version = "0.1.0"

repositories {
    google()
    mavenCentral()
}

val repoRoot = rootDir.parentFile.parentFile
val ffiLibDir = repoRoot.resolve("target/debug")

dependencies {
    implementation(compose.desktop.currentOs)
    implementation(compose.material3)
    implementation("net.java.dev.jna:jna:5.15.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-swing:1.9.0")
    testImplementation(kotlin("test"))
}

compose.desktop {
    application {
        mainClass = "org.nemo.MainKt"
        jvmArgs += "-Djna.library.path=${ffiLibDir.absolutePath}"
    }
}

tasks.test {
    useJUnitPlatform()
    systemProperty("jna.library.path", ffiLibDir.absolutePath)
    environment("jna.library.path", ffiLibDir.absolutePath)
}

compose.desktop {
    application {
        mainClass = "org.nemo.MainKt"
        jvmArgs += "-Djna.library.path=${ffiLibDir.absolutePath}"
    }
}

kotlin {
    jvmToolchain(21)
}
