plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

// Standalone Wear OS node: runs the same Rust engine as the phone (:client),
// provisioned from the phone over the Wearable Data Layer or by an invite code.
android {
    namespace = "ro.titi.wear"
    compileSdk = 37

    defaultConfig {
        // Same package + signing key as the phone app: required for the Data Layer (WO-G7).
        applicationId = "ro.titi.app"
        minSdk = 30 // Wear OS 3+
        targetSdk = 36
        // Watch builds live in their own versionCode range (Play multi-APK rule).
        versionCode = 200_000_001
        versionName = "0.1.0"
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
        checkDependencies = false
    }
}

dependencies {
    implementation(project(":client"))

    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.foundation)
    implementation(libs.compose.animation)
    implementation(libs.compose.material.icons.extended)
    implementation(libs.compose.ui.tooling.preview)
    debugImplementation(libs.compose.ui.tooling)
    implementation(libs.wear.compose.material3)
    implementation(libs.wear.compose.foundation)
    implementation(libs.wear.compose.navigation)
    implementation(libs.wear.compose.ui.tooling)

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.service)
    implementation(libs.androidx.lifecycle.process)

    implementation(libs.wear.tiles)
    implementation(libs.wear.protolayout)
    implementation(libs.wear.protolayout.material3)
    implementation(libs.wear.complications.ktx)
    implementation(libs.wear.ongoing)
    implementation(libs.wear.input)
    implementation(libs.play.services.wearable)
    implementation(libs.kotlinx.coroutines.play.services)
}
