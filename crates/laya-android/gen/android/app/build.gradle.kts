import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

android {
    compileSdk = 36
    namespace = "dev.laya.mobile"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "dev.laya.mobile"
        minSdk = 24
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    // Release signing material comes from CI secrets, never from checked-in files.
    val releaseKeystore = providers.environmentVariable("LAYA_ANDROID_KEYSTORE").orNull
    if (releaseKeystore != null) {
        signingConfigs {
            create("release") {
                storeFile = file(releaseKeystore)
                storePassword = providers.environmentVariable("LAYA_ANDROID_STORE_PASSWORD").orNull
                    ?: error("LAYA_ANDROID_STORE_PASSWORD is required to sign a release")
                keyAlias = providers.environmentVariable("LAYA_ANDROID_KEY_ALIAS").orNull
                    ?: error("LAYA_ANDROID_KEY_ALIAS is required to sign a release")
                keyPassword = providers.environmentVariable("LAYA_ANDROID_KEY_PASSWORD").orNull
                    ?: error("LAYA_ANDROID_KEY_PASSWORD is required to sign a release")
            }
        }
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            if (releaseKeystore != null) {
                signingConfig = signingConfigs.getByName("release")
            }
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
    }
}

// ONNX Runtime is linked into liblaya_android.so but requires the NDK C++
// runtime. Rust-only Android packaging does not add libc++_shared.so for us.
val stageArm64CxxRuntime = tasks.register<Copy>("stageArm64CxxRuntime") {
    val ndkHome = providers.environmentVariable("NDK_HOME").orNull
        ?: error("NDK_HOME must point to the NDK used for the ARM64 Rust build")
    val runtime = fileTree("$ndkHome/toolchains/llvm/prebuilt") {
        include("*/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so")
    }.singleFile
    from(runtime)
    into(layout.buildDirectory.dir("generated/arm64-cxx-jni/arm64-v8a"))
}
android.sourceSets.getByName("main").jniLibs.srcDir(layout.buildDirectory.dir("generated/arm64-cxx-jni"))
tasks.matching { it.name.startsWith("mergeArm64") && it.name.endsWith("JniLibFolders") }
    .configureEach { dependsOn(stageArm64CxxRuntime) }

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")