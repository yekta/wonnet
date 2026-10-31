plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

// Push needs the Firebase project's file, which CI writes from a secret; without it the app
// builds and runs, only without push.
if (file("google-services.json").exists()) apply(plugin = "com.google.gms.google-services")

val root = rootDir.resolve("../..")

/** The workspace's version, which scripts/release-version.sh writes into Cargo.toml. */
val cargoVersion: String = root.resolve("Cargo.toml").readLines()
    .first { it.startsWith("version = ") }.substringAfter('"').substringBefore('"')

/** 1.2.3 is 1_02_0003: each release's code is above the one before. */
val cargoVersionCode: Int = cargoVersion.split('.').map { it.toIntOrNull() ?: 0 }.let { (major, minor, patch) ->
    major * 1_000_000 + minor * 10_000 + patch
}

/** The Kotlin half of rustls-platform-verifier must be the version of its Rust half in Cargo.lock. */
val verifierVersion: String = root.resolve("Cargo.lock").readLines().let { lines ->
    val at = lines.indexOfFirst { it.trim() == "name = \"rustls-platform-verifier-android\"" }
    lines.drop(at + 1).first { it.startsWith("version = ") }.substringAfter('"').substringBefore('"')
}

/** A release's signing, from the variables CI is given; without them a release is signed for debugging. */
val keystore: String? = System.getenv("ANDROID_KEYSTORE_PATH")?.takeIf { it.isNotEmpty() }

android {
    namespace = "app.wonnet"
    compileSdk = 37

    defaultConfig {
        applicationId = "app.wonnet.wonnet"
        minSdk = 29
        targetSdk = 37
        versionName = cargoVersion
        versionCode = cargoVersionCode
        val serverUrl = System.getenv("MAIL_SERVER_URL") ?: "https://api.wonnet.app"
        buildConfigField("String", "SERVER_URL", "\"$serverUrl\"")
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }

    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEY_ALIAS")
                keyPassword = System.getenv("ANDROID_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    sourceSets["main"].jniLibs.directories.add("build/jniLibs")
    // Lucide's font, as the Apple apps have it.
    sourceSets["main"].assets.directories.add(root.resolve("packages/apple/Sources/MailUI/Fonts").path)

    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
    }
}

kotlin {
    jvmToolchain(21)
}

/** Builds the Rust core into build/jniLibs. `-Pwonnet.skipCore` keeps the one built last. */
val skipCore = providers.gradleProperty("wonnet.skipCore").isPresent
val buildCore = tasks.register<Exec>("buildCore") {
    workingDir = rootDir
    commandLine("scripts/build-core.sh")
    enabled = !skipCore
}
tasks.named("preBuild") { dependsOn(buildCore) }

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.ui)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    implementation(libs.androidx.core)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.browser)
    implementation(libs.androidx.lifecycle.process)
    implementation(libs.coroutines)
    implementation(libs.serialization.json)
    implementation(libs.firebase.messaging)
    // Firebase brings an old Fragment, which the Activity Result API needs at 1.3 or later.
    implementation(libs.fragment)
    // The version is the Rust half's in Cargo.lock, which the catalog can't read.
    //noinspection UseTomlInstead
    implementation("org.rustls:rustls-platform-verifier:$verifierVersion")
}
