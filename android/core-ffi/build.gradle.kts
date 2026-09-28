import org.gradle.api.tasks.Exec

plugins {
    alias(libs.plugins.android.library)
}

val repoRoot = rootProject.projectDir.parentFile
val cargoManifest = repoRoot.resolve("core/Cargo.toml")
val jniOut = layout.buildDirectory.dir("rustJniLibs")
val bindingsOut = layout.buildDirectory.dir("generated/uniffi/kotlin")
val buildRust = providers.gradleProperty("titi.buildRust").getOrElse("true").toBoolean()

fun abisFor(variant: String): List<String> {
    val key = if (variant.contains("release", ignoreCase = true)) "titi.rustAbis.release" else "titi.rustAbis.debug"
    return providers.gradleProperty(key).getOrElse("arm64-v8a").split(',').map { it.trim() }.filter { it.isNotEmpty() }
}

val ndkVer = "28.2.13676358"

val hostLibName = when {
    org.gradle.internal.os.OperatingSystem.current().isWindows -> "titi_ffi.dll"
    org.gradle.internal.os.OperatingSystem.current().isMacOsX -> "libtiti_ffi.dylib"
    else -> "libtiti_ffi.so"
}
val hostLibFile = repoRoot.resolve("core/target/debug/$hostLibName")
val rustSources = fileTree(repoRoot.resolve("core")) { include("**/*.rs", "**/Cargo.toml", "**/build.rs", "Cargo.lock", ".cargo/config.toml"); exclude("target/**") }

/** libopus is built by cmake inside the cargo build; it needs the NDK toolchain file + ninja. */
fun Exec.rustEnv() {
    val sdk = System.getenv("ANDROID_HOME") ?: System.getenv("ANDROID_SDK_ROOT")
        ?: (rootProject.file("local.properties").takeIf { it.exists() }?.readLines()
            ?.firstOrNull { it.startsWith("sdk.dir=") }?.substringAfter("=")?.replace("\\:", ":")?.replace("\\\\", "\\"))
        ?: error("ANDROID_HOME not set and no local.properties sdk.dir")
    val ndk = File(sdk, "ndk/$ndkVer")
    environment("ANDROID_NDK_HOME", ndk.absolutePath)
    environment("CMAKE_TOOLCHAIN_FILE", File(ndk, "build/cmake/android.toolchain.cmake").absolutePath)
    val isWin = org.gradle.internal.os.OperatingSystem.current().isWindows
    val sdkNinja = File(sdk, "cmake/3.22.1/bin/" + if (isWin) "ninja.exe" else "ninja")
    if (sdkNinja.exists()) environment("CMAKE_MAKE_PROGRAM", sdkNinja.absolutePath)
    // otherwise rely on `ninja` on PATH
}

android {
    namespace = "ro.titi.core"
    compileSdk = 37
    ndkVersion = ndkVer
    defaultConfig {
        minSdk = 26
        consumerProguardFiles("consumer-rules.pro")
    }
    buildTypes {
        release { isMinifyEnabled = false }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    sourceSets {
        getByName("main") {
            jniLibs.directories.add(jniOut.get().asFile.absolutePath)
            kotlin.directories.add(bindingsOut.get().asFile.absolutePath)
        }
    }
}

dependencies {
    implementation(libs.jna) { artifact { type = "aar" } }
    implementation(libs.kotlinx.coroutines.android)
}

// ---- Rust build -------------------------------------------------------------

val cargoNdkDebug = tasks.register<Exec>("cargoNdkDebug") {
    group = "rust"
    description = "cargo ndk build of titi-ffi (debug)"
    enabled = buildRust // a plain Boolean: an onlyIf{} lambda captures the script and breaks the configuration cache
    workingDir = repoRoot.resolve("core")
    rustEnv()
    val abis = abisFor("debug").flatMap { listOf("-t", it) }
    commandLine(listOf("cargo", "ndk", "-o", jniOut.get().asFile.absolutePath, "--platform", "26") + abis + listOf("build", "-p", "titi-ffi"))
    inputs.files(rustSources)
    inputs.property("abis", abis)
    outputs.dir(jniOut)
}

val cargoNdkRelease = tasks.register<Exec>("cargoNdkRelease") {
    group = "rust"
    description = "cargo ndk build of titi-ffi (release)"
    enabled = buildRust
    workingDir = repoRoot.resolve("core")
    rustEnv()
    val abis = abisFor("release").flatMap { listOf("-t", it) }
    commandLine(listOf("cargo", "ndk", "-o", jniOut.get().asFile.absolutePath, "--platform", "26") + abis + listOf("build", "-p", "titi-ffi", "--release"))
    inputs.files(rustSources)
    inputs.property("abis", abis)
    outputs.dir(jniOut)
}

// Kotlin bindings are generated from the *host* cdylib (library mode) so no
// UDL is needed; proc-macro metadata is read from the compiled library.
val hostLib = tasks.register<Exec>("cargoBuildHostFfi") {
    group = "rust"
    enabled = buildRust
    workingDir = repoRoot.resolve("core")
    commandLine("cargo", "build", "-p", "titi-ffi", "--features", "cli")
    inputs.files(rustSources)
    // the single cdylib — declaring all of target/debug (4.5 GB) made Gradle fingerprint it every build
    outputs.file(hostLibFile)
}

val uniffiBindgen = tasks.register<Exec>("uniffiBindgen") {
    group = "rust"
    description = "Generate Kotlin bindings for titi-ffi"
    enabled = buildRust
    dependsOn(hostLib)
    workingDir = repoRoot.resolve("core")
    commandLine(
        "cargo", "run", "-p", "titi-ffi", "--features", "cli", "--bin", "uniffi-bindgen", "--",
        "generate", "--library", "target/debug/$hostLibName", "--language", "kotlin",
        "--out-dir", bindingsOut.get().asFile.absolutePath, "--no-format",
    )
    // re-run whenever the cdylib (i.e. the exported API) changes
    inputs.file(hostLibFile)
    outputs.dir(bindingsOut)
}

// Debug and release write to the same jniLibs dir, so they must not run in the
// same invocation; wire each variant's merge task to its own cargo task.
tasks.matching { it.name == "mergeDebugJniLibFolders" }.configureEach { dependsOn(cargoNdkDebug) }
tasks.matching { it.name == "mergeReleaseJniLibFolders" }.configureEach { dependsOn(cargoNdkRelease) }
tasks.matching { (it.name.startsWith("compile") && it.name.contains("Kotlin")) || it.name.startsWith("extract") && it.name.endsWith("Annotations") }.configureEach { dependsOn(uniffiBindgen) }
