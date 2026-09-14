plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "ro.titi.app"
    compileSdk = 37

    defaultConfig {
        applicationId = "ro.titi.app"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        vectorDrawables.useSupportLibrary = true
    }
    androidResources {
        localeFilters += listOf("en", "ro")
    }

    flavorDimensions += "distribution"
    productFlavors {
        create("gms") {
            dimension = "distribution"
            isDefault = true
        }
        create("foss") {
            dimension = "distribution"
            applicationIdSuffix = ".foss"
        }
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
            isDebuggable = true
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
        compose = true
        buildConfig = true
    }
    packaging {
        resources.excludes += setOf("META-INF/*.kotlin_module", "META-INF/LICENSE*", "META-INF/AL2.0", "META-INF/LGPL2.1")
        jniLibs.useLegacyPackaging = false
    }
    bundle {
        language.enableSplit = false
    }
}

dependencies {
    implementation(project(":core-ffi"))

    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.ui.graphics)
    implementation(libs.compose.ui.tooling.preview)
    implementation(libs.compose.foundation)
    implementation(libs.compose.animation)
    implementation(libs.compose.material3)
    implementation(libs.compose.material.icons.extended)
    debugImplementation(libs.compose.ui.tooling)

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.service)
    implementation(libs.androidx.navigation3.runtime)
    implementation(libs.androidx.navigation3.ui)
    implementation(libs.androidx.datastore.preferences)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.kotlinx.serialization.json)

    implementation(libs.camerax.core)
    implementation(libs.camerax.camera2)
    implementation(libs.camerax.lifecycle)
    implementation(libs.camerax.view)
    implementation(libs.camerax.mlkit.vision)
    implementation(libs.mlkit.barcode)
    implementation(libs.zxing.core)
    implementation(libs.okhttp)
    implementation(libs.androidx.media)

    implementation(libs.glance.appwidget)
    implementation(libs.glance.material3)

    "gmsImplementation"(libs.play.services.nearby)
    "gmsImplementation"(libs.kotlinx.coroutines.play.services)

    // Glance/ML Kit drag in work-runtime 2.7.1 + room 2.2.5, which crash under
    // R8 full mode ("Failed to create an instance of WorkDatabase"). Pin current.
    constraints {
        implementation("androidx.work:work-runtime:2.11.2")
        implementation("androidx.work:work-runtime-ktx:2.11.2")
        implementation("androidx.room:room-runtime:2.8.5")
    }

    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.test.ext.junit)
    androidTestImplementation(libs.androidx.test.espresso)
    androidTestImplementation(platform(libs.compose.bom))
    androidTestImplementation(libs.compose.ui.test.junit4)
    debugImplementation(libs.compose.ui.test.manifest)
}
