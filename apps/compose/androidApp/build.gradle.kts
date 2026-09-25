plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.compose.multiplatform)
}

android {
    namespace = "org.nemo"
    compileSdk = 37
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
    // Local/dev only: sign release with the debug key so sideload works.
    // Replace with a real release keystore before Play / wide distribution.
    buildTypes {
        release {
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    // One APK per CPU ABI so phones don't ship the emulator .so (and vice versa).
    splits {
        abi {
            isEnable = true
            reset()
            include("arm64-v8a", "x86_64")
            isUniversalApk = false
        }
    }
    packaging {
        jniLibs {
            keepDebugSymbols += setOf("**/libnemo_ffi.so")
        }
    }
}

dependencies {
    implementation(project(":composeApp"))
}

tasks.named("preBuild").configure {
    dependsOn(":composeApp:cargoNdkFfi")
}
