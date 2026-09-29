plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

// Android TV / Google TV node: the same Rust engine as the phone (:client), used as a
// room speaker. Joins by code; talks only when the TV has a real microphone input.
android {
    namespace = "ro.titi.tv"
    compileSdk = 37

    defaultConfig {
        applicationId = "ro.titi.app"
        minSdk = 26
        targetSdk = 36
        // TV builds live in their own versionCode range (Play multi-APK rule).
        versionCode = 300_000_000 + (System.getenv("TITI_VERSION_CODE")?.toInt() ?: 1)
        versionName = System.getenv("TITI_VERSION_NAME") ?: "0.1.0"
        // only ABIs we build titi_ffi for; drops JNA's x86/mips/armeabi and ML Kit's x86 blobs
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a") }
    }
    androidResources {
        localeFilters += listOf("en", "ro")
    }

    signingConfigs {
        create("release") {
            val ksPath = System.getenv("TITI_KEYSTORE") ?: project.findProperty("titi.keystore") as String?
            if (ksPath != null && file(ksPath).exists()) {
                storeFile = file(ksPath)
                storePassword = System.getenv("TITI_KEYSTORE_PASSWORD") ?: project.findProperty("titi.keystorePassword") as String?
                keyAlias = System.getenv("TITI_KEY_ALIAS") ?: project.findProperty("titi.keyAlias") as String? ?: "upload"
                keyPassword = System.getenv("TITI_KEY_PASSWORD") ?: project.findProperty("titi.keyPassword") as String?
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.getByName("release").takeIf { it.storeFile != null }
        }
        debug {
            applicationIdSuffix = ".debug"
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        compose = true
    }
    packaging {
        resources.excludes += setOf("META-INF/*.kotlin_module", "META-INF/LICENSE*", "META-INF/AL2.0", "META-INF/LGPL2.1")
    }
    lint {
        abortOnError = true
        // tests are compiled + run by the gates; linting them re-analyzes ~20 s per module for no findings
        ignoreTestSources = true
        checkDependencies = false
    }
}

dependencies {
    implementation(project(":client"))

    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.foundation)
    implementation(libs.compose.animation)
    implementation(libs.tv.material)

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.service)
    implementation(libs.androidx.lifecycle.process)

    implementation(libs.zxing.core)
}
