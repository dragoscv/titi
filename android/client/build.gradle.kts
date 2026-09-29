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
        // tests are compiled + run by the gates; linting them re-analyzes ~20 s per module for no findings
        ignoreTestSources = true
    }
}

dependencies {
    api(project(":core-ffi"))
    api(libs.kotlinx.coroutines.android)
    api(libs.androidx.datastore.preferences)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.process)
    implementation(libs.okhttp)
    // icons: material-icons-core + the few extended icons we use, vendored under src/main/kotlin/androidx (scripts/vendor-icons.mjs)
    api(platform(libs.compose.bom))
    api(libs.compose.material.icons.core)
}
