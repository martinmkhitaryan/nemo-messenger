plugins {
    alias(libs.plugins.kotlin.multiplatform) apply false
    alias(libs.plugins.compose.multiplatform) apply false
    alias(libs.plugins.kotlin.compose) apply false
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.android.kotlin.multiplatform.library) apply false
    alias(libs.plugins.ktlint) apply false
    alias(libs.plugins.detekt) apply false
}

// Keep historical task names working from apps/compose (scripts + docs).
tasks.register("run") {
    group = "application"
    dependsOn(":composeApp:run")
}

tasks.register("runRelease") {
    group = "application"
    dependsOn(":composeApp:runRelease")
}

tasks.register("desktopTest") {
    group = "verification"
    dependsOn(":composeApp:desktopTest")
}

tasks.register("ktlintCheck") {
    group = "verification"
    dependsOn(":composeApp:ktlintCheck")
}

tasks.register("detekt") {
    group = "verification"
    dependsOn(":composeApp:detekt")
}

tasks.register("assembleDebug") {
    group = "build"
    dependsOn(":androidApp:assembleDebug")
}

tasks.register("assembleRelease") {
    group = "build"
    dependsOn(":androidApp:assembleRelease")
}

tasks.register("installDebug") {
    group = "install"
    dependsOn(":androidApp:installDebug")
}

tasks.register("installRelease") {
    group = "install"
    dependsOn(":androidApp:installRelease")
}

tasks.register("packageDeb") {
    group = "compose desktop"
    dependsOn(":composeApp:packageDeb")
}

tasks.register("connectedDebugAndroidTest") {
    group = "verification"
    dependsOn(":composeApp:connectedAndroidDeviceTest")
}

tasks.register("connectedAndroidDeviceTest") {
    group = "verification"
    dependsOn(":composeApp:connectedAndroidDeviceTest")
}
