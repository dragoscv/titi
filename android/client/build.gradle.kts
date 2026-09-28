plugins {
    alias(libs.plugins.android.library)
}

// Shared radio client for phone (:app) and watch (:wear): Rust engine host,
// audio, transports and prefs. No UI, no Compose, no GMS.
android {
    namespace = "ro.titi.client"
    compileSdk = 37
    defaultConfig {
        minSdk = 26
        consumerProguardFiles("consumer-rules.pro")
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    lint {
        abortOnError = true
    }
}

dependencies {
    api(project(":core-ffi"))
    api(libs.kotlinx.coroutines.android)
    api(libs.androidx.datastore.preferences)
    implementation(libs.androidx.core.ktx)
    implementation(libs.okhttp)
}
