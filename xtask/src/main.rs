use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Error, Debug)]
enum XtaskError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Command failed: {0}")]
    CommandFailed(String),
    #[error("Missing tool or path: {0}")]
    MissingPath(String),
}

type Result<T> = std::result::Result<T, XtaskError>;

fn main() {
    if let Err(err) = try_main() {
        eprintln!("xtask error: {err}");
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");

    let root = workspace_root()?;
    match cmd {
        "bindgen" => run_bindgen(&root),
        "build-aar" => {
            run_bindgen(&root)?;
            build_aar(&root)
        }
        "spike-android" => {
            run_bindgen(&root)?;
            build_aar(&root)?;
            build_and_patch_dioxus_android(&root, false)
        }
        "release-apk" => {
            run_bindgen(&root)?;
            build_aar(&root)?;
            build_and_patch_dioxus_android(&root, true)
        }
        _ => {
            println!("Usage: cargo run -p xtask -- <bindgen|build-aar|spike-android|release-apk>");
            Ok(())
        }
    }
}

fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .map_err(|_| XtaskError::MissingPath("CARGO_MANIFEST_DIR".to_string()))?;
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| XtaskError::MissingPath("Workspace root parent".to_string()))
}

fn find_android_sdk() -> Result<PathBuf> {
    if let Ok(sdk) = env::var("ANDROID_HOME").or_else(|_| env::var("ANDROID_SDK_ROOT")) {
        let p = PathBuf::from(sdk);
        if p.exists() {
            return Ok(p);
        }
    }
    let home = env::var("HOME").map_err(|_| XtaskError::MissingPath("HOME".to_string()))?;
    let fallback = PathBuf::from(home).join("Android/Sdk");
    if fallback.exists() {
        return Ok(fallback);
    }
    Err(XtaskError::MissingPath("Android SDK directory".to_string()))
}

fn find_ndk(sdk: &Path) -> Result<PathBuf> {
    for var in [
        "ANDROID_NDK_HOME",
        "ANDROID_NDK_ROOT",
        "ANDROID_NDK_LATEST_HOME",
        "NDK_HOME",
    ] {
        if let Ok(ndk) = env::var(var) {
            let p = PathBuf::from(ndk);
            if p.exists() {
                return Ok(p);
            }
        }
    }
    let ndk_root = sdk.join("ndk");
    if ndk_root.exists() {
        let mut entries: Vec<PathBuf> = fs::read_dir(&ndk_root)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        entries.sort();
        if let Some(latest) = entries.pop() {
            return Ok(latest);
        }
    }
    Err(XtaskError::MissingPath("Android NDK directory".to_string()))
}

fn find_jdk17() -> Result<PathBuf> {
    if let Ok(jh) = env::var("JAVA_HOME") {
        let p = PathBuf::from(jh);
        if p.join("bin/javac").exists() {
            return Ok(p);
        }
    }
    let home = env::var("HOME").map_err(|_| XtaskError::MissingPath("HOME".to_string()))?;
    for candidate in [
        PathBuf::from(&home).join("jdk17"),
        PathBuf::from(&home).join(".sdkman/candidates/java/current"),
        PathBuf::from(&home).join(".sdkman/tmp/out/amazon-corretto-17.0.20.8.1-linux-x64"),
    ] {
        if candidate.join("bin/javac").exists() {
            return Ok(candidate);
        }
    }
    Err(XtaskError::MissingPath("JDK 17 JAVA_HOME".to_string()))
}

fn find_gradle_bin() -> Result<PathBuf> {
    // 1. Check explicit GRADLE_BIN environment variable
    if let Ok(bin) = env::var("GRADLE_BIN") {
        let p = PathBuf::from(bin);
        if p.exists() {
            return Ok(p);
        }
    }
    // 2. Check system PATH
    if let Ok(path) = env::var("PATH") {
        for dir in env::split_paths(&path) {
            let candidate = dir.join("gradle");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    // 3. Fallback to cached gradle wrappers in ~/.gradle/wrapper/dists
    if let Ok(home) = env::var("HOME") {
        let dists = PathBuf::from(home).join(".gradle/wrapper/dists");
        if dists.exists() {
            if let Ok(entries) = fs::read_dir(&dists) {
                for version_dir in entries.flatten() {
                    let base = version_dir.path();
                    if let Ok(subs) = fs::read_dir(&base) {
                        for sub in subs.flatten() {
                            if let Ok(inners) = fs::read_dir(sub.path()) {
                                for inner in inners.flatten() {
                                    let bin = inner.path().join("bin/gradle");
                                    if bin.exists() {
                                        return Ok(bin);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Err(XtaskError::MissingPath(
        "Gradle binary (not found in PATH, GRADLE_BIN or ~/.gradle/wrapper/dists)".to_string(),
    ))
}

fn run_bindgen(root: &Path) -> Result<()> {
    println!("==> [1/4] Building crabcheck-core-ffi for host and generating Kotlin UniFFI bindings...");
    let status = Command::new("cargo")
        .current_dir(root)
        .args(["build", "-p", "crabcheck-core-ffi"])
        .status()?;
    if !status.success() {
        return Err(XtaskError::CommandFailed(
            "cargo build -p crabcheck-core-ffi".to_string(),
        ));
    }

    let lib_path = root.join("target/debug/libcrabcheck_core_ffi.so");
    let out_dir = root.join("android/alarm-lib/src/main/java");

    let status = Command::new("cargo")
        .current_dir(root)
        .args([
            "run",
            "-p",
            "crabcheck-core-ffi",
            "--bin",
            "uniffi-bindgen",
            "--",
            "generate",
            "--library",
            &lib_path.to_string_lossy(),
            "--language",
            "kotlin",
            "--out-dir",
            &out_dir.to_string_lossy(),
        ])
        .status()?;
    if !status.success() {
        return Err(XtaskError::CommandFailed(
            "uniffi-bindgen generate".to_string(),
        ));
    }

    // Patch generated loadIndirect() to fallback from "crabcheck_core_ffi" (host JUnit) to "dioxusmain" (Android APK)
    let kt_binding = out_dir.join("uniffi/crabcheck_core_ffi/crabcheck_core_ffi.kt");
    if kt_binding.exists() {
        let content = fs::read_to_string(&kt_binding)?;
        let patched = content.replace(
            "return Native.load<Lib>(findLibraryName(componentName), Lib::class.java)",
            "return try { Native.load<Lib>(findLibraryName(componentName), Lib::class.java) } catch (_: Throwable) { Native.load<Lib>(\"dioxusmain\", Lib::class.java) }",
        );
        fs::write(&kt_binding, patched)?;
    }

    Ok(())
}

fn build_aar(root: &Path) -> Result<()> {
    println!("==> [2/4] Building and testing android/alarm-lib AAR...");
    let sdk = find_android_sdk()?;
    let jdk = find_jdk17()?;
    let gradle = find_gradle_bin()?;
    let alarm_lib_dir = root.join("android/alarm-lib");
    let host_lib_dir = root.join("target/debug");

    let status = Command::new(&gradle)
        .current_dir(&alarm_lib_dir)
        .env("ANDROID_HOME", &sdk)
        .env("ANDROID_SDK_ROOT", &sdk)
        .env("JAVA_HOME", &jdk)
        .arg(format!(
            "-Djna.library.path={}",
            host_lib_dir.to_string_lossy()
        ))
        .arg(format!(
            "-Djava.library.path={}",
            host_lib_dir.to_string_lossy()
        ))
        .args(["testDebugUnitTest", "assembleRelease"])
        .status()?;

    if !status.success() {
        return Err(XtaskError::CommandFailed(
            "gradle testDebugUnitTest assembleRelease in android/alarm-lib".to_string(),
        ));
    }
    Ok(())
}

fn build_and_patch_dioxus_android(root: &Path, is_release: bool) -> Result<()> {
    let mode_str = if is_release { "release" } else { "debug" };
    println!("==> [3/4] Running dx build ({mode_str}) --platform android for apps/app...");
    let sdk = find_android_sdk()?;
    let ndk = find_ndk(&sdk)?;
    let jdk = find_jdk17()?;
    let app_dir = root.join("apps/app");

    let path_env = env::var("PATH").unwrap_or_default();
    let new_path = format!(
        "{}:{}:{}",
        jdk.join("bin").to_string_lossy(),
        sdk.join("platform-tools").to_string_lossy(),
        path_env
    );

    for arch in ["arm64", "x64"] {
        let mut cmd = Command::new("dx");
        cmd.current_dir(&app_dir)
            .env("ANDROID_HOME", &sdk)
            .env("ANDROID_SDK_ROOT", &sdk)
            .env("ANDROID_NDK_HOME", &ndk)
            .env("NDK_HOME", &ndk)
            .env("JAVA_HOME", &jdk)
            .env("PATH", &new_path);

        if is_release {
            cmd.args(["build", "--release", "--platform", "android", "--arch", arch]);
        } else {
            cmd.args(["build", "--platform", "android", "--arch", arch]);
        }

        let dx_status = cmd.status()?;

        if !dx_status.success() {
            return Err(XtaskError::CommandFailed(format!(
                "dx build ({mode_str}) --platform android --arch {arch}"
            )));
        }
    }

    let gen_android = find_dx_android_dir(root, is_release)?;
    println!(
        "==> [4/4] Patching generated Android project at {} and assembling APK...",
        gen_android.display()
    );

    patch_dx_android_project(root, &gen_android)?;

    let gradlew = gen_android.join("gradlew");
    let assemble_task = if is_release { "assembleRelease" } else { "assembleDebug" };

    let assemble_status = Command::new(&gradlew)
        .current_dir(&gen_android)
        .env("ANDROID_HOME", &sdk)
        .env("ANDROID_SDK_ROOT", &sdk)
        .env("ANDROID_NDK_HOME", &ndk)
        .env("JAVA_HOME", &jdk)
        .env("PATH", &new_path)
        .arg(assemble_task)
        .status()?;

    if !assemble_status.success() {
        return Err(XtaskError::CommandFailed(format!(
            "gradlew {assemble_task} on patched Dioxus project"
        )));
    }

    let apk_path = if is_release {
        let rel_apk = gen_android.join("app/build/outputs/apk/release/app-release.apk");
        let candidate = if !rel_apk.exists() {
            gen_android.join("app/build/outputs/apk/release/app-release-unsigned.apk")
        } else {
            rel_apk
        };
        let target_release = root.join("target/crabcheck-v0.1.0-release.apk");
        if candidate.exists() {
            fs::copy(&candidate, &target_release)?;
            println!("==> Copied official release APK to: {}", target_release.display());
            
            // Verify signature using apksigner if available
            let bt_root = sdk.join("build-tools");
            if bt_root.exists() {
                if let Ok(entries) = fs::read_dir(&bt_root) {
                    for entry in entries.flatten() {
                        let apksigner = entry.path().join("apksigner");
                        if apksigner.exists() {
                            println!("==> Verifying release APK signature with {}...", apksigner.display());
                            let _ = Command::new(&apksigner)
                                .args(["verify", "--verbose", "--print-certs", target_release.to_str().unwrap()])
                                .status();
                            break;
                        }
                    }
                }
            }
        }
        target_release
    } else {
        gen_android.join("app/build/outputs/apk/debug/app-debug.apk")
    };

    println!("==> SUCCESS! Patched APK generated at: {}", apk_path.display());
    Ok(())
}

fn find_dx_android_dir(root: &Path, is_release: bool) -> Result<PathBuf> {
    let target_dx = root.join("target/dx");
    let modes = if is_release {
        ["release", "debug"]
    } else {
        ["debug", "release"]
    };
    if target_dx.exists() {
        for entry in fs::read_dir(&target_dx)? {
            let p = entry?.path();
            for mode in modes {
                let candidate = p.join(mode).join("android/app");
                if candidate.join("gradlew").exists() {
                    return Ok(candidate);
                }
            }
        }
    }
    Err(XtaskError::MissingPath(
        "Generated target/dx/<app>/<mode>/android/app directory".to_string(),
    ))
}

fn patch_dx_android_project(root: &Path, gen_android: &Path) -> Result<()> {
    let libs_dir = gen_android.join("app/libs");
    fs::create_dir_all(&libs_dir)?;

    let aar_src = root.join("android/alarm-lib/build/outputs/aar/alarm-lib-release.aar");
    let aar_dst = libs_dir.join("alarm-lib.aar");
    fs::copy(&aar_src, &aar_dst)?;

    // Copy brand logo to Android res/drawable
    let res_drawable = gen_android.join("app/src/main/res/drawable");
    fs::create_dir_all(&res_drawable)?;
    let logo_src = root.join("assets/crabcheck.png");
    if logo_src.exists() {
        let _ = fs::copy(&logo_src, res_drawable.join("crabcheck_logo.png"));
    }

    // Copy official release keystore if present
    let keystore_src = root.join("android/keystore/crabcheck-release.jks");
    if keystore_src.exists() {
        let keystore_dst = gen_android.join("app/crabcheck-release.jks");
        fs::copy(&keystore_src, &keystore_dst)?;
    }

    let gradle_kts = gen_android.join("app/build.gradle.kts");
    if gradle_kts.exists() {
        let mut content = fs::read_to_string(&gradle_kts)?;
        content = content.replace("minSdk = 24", "minSdk = 26");
        content = content.replace("minSdk = 29", "minSdk = 26");
        content = content.replace("compileSdk = 33", "compileSdk = 34");
        content = content.replace("targetSdk = 33", "targetSdk = 34");

        // Enforce clean ABI filtering so only supported complete architectures are packaged
        let abi_filters_block = r#"
        ndk {
            abiFilters.clear()
            abiFilters.addAll(listOf("arm64-v8a", "x86_64"))
        }
"#;
        if !content.contains("abiFilters.addAll") {
            content = content.replace("defaultConfig {", &format!("defaultConfig {{{abi_filters_block}"));
        }

        // Exclude mismatched third-party 32-bit libraries (JNA, etc.)
        let jni_packaging_block = r#"
    packaging {
        jniLibs {
            excludes.addAll(listOf(
                "lib/armeabi/**",
                "lib/armeabi-v7a/**",
                "lib/mips/**",
                "lib/mips64/**",
                "lib/x86/**"
            ))
        }
    }
"#;
        if !content.contains("lib/armeabi/**") {
            content = content.replace("android {", &format!("android {{{jni_packaging_block}"));
        }

        let aar_dep = "implementation(files(\"libs/alarm-lib.aar\"))";
        if !content.contains(aar_dep) {
            content = content.replace(
                "dependencies {",
                "dependencies {\n    implementation(files(\"libs/alarm-lib.aar\"))\n    implementation(\"net.java.dev.jna:jna:5.14.0@aar\")",
            );
        }
        if !content.contains("com.google.mlkit:image-labeling") {
            let mlkit_deps = concat!(
                "dependencies {\n",
                "    implementation(\"androidx.camera:camera-core:1.3.4\")\n",
                "    implementation(\"androidx.camera:camera-camera2:1.3.4\")\n",
                "    implementation(\"androidx.camera:camera-lifecycle:1.3.4\")\n",
                "    implementation(\"androidx.camera:camera-view:1.3.4\")\n",
                "    implementation(\"com.google.mlkit:image-labeling:17.0.8\")"
            );
            content = content.replace("dependencies {", mlkit_deps);
        }

        if keystore_src.exists() {
            let signing_block = r#"
    signingConfigs {
        create("release") {
            storeFile = file("crabcheck-release.jks")
            storePassword = "crabcheck2026"
            keyAlias = "crabcheck"
            keyPassword = "crabcheck2026"
            enableV1Signing = true
            enableV2Signing = true
            enableV3Signing = true
            enableV4Signing = false
        }
    }
"#;
            if !content.contains("create(\"release\")") {
                content = content.replace("android {", &format!("android {{{signing_block}"));
            }
            content = content.replace(
                "signingConfig = signingConfigs.getByName(\"debug\")",
                "signingConfig = signingConfigs.getByName(\"release\")",
            );
            if !content.contains("signingConfig = signingConfigs.getByName(\"release\")") {
                content = content.replace(
                    "getByName(\"release\") {",
                    "getByName(\"release\") {\n            signingConfig = signingConfigs.getByName(\"release\")",
                );
            }
        } else if !content.contains("signingConfig = signingConfigs.getByName(\"debug\")") {
            content = content.replace(
                "getByName(\"release\") {",
                "getByName(\"release\") {\n            signingConfig = signingConfigs.getByName(\"debug\")",
            );
        }
        fs::write(&gradle_kts, content)?;
    }

    let manifest_path = gen_android.join("app/src/main/AndroidManifest.xml");
    if manifest_path.exists() {
        let mut manifest = fs::read_to_string(&manifest_path)?;
        if !manifest.contains("com.crabcheck.alarmlib.LockActivity") {
            let perms = r#"    <uses-permission android:name="android.permission.SCHEDULE_EXACT_ALARM" />
    <uses-permission android:name="android.permission.USE_EXACT_ALARM" />
    <uses-permission android:name="android.permission.USE_FULL_SCREEN_INTENT" />
    <uses-permission android:name="android.permission.WAKE_LOCK" />
    <uses-permission android:name="android.permission.POST_NOTIFICATIONS" />
    <application"#;
            manifest = manifest.replace("    <application", perms);

            let components = r#"        <receiver
            android:name="com.crabcheck.alarmlib.AlarmReceiver"
            android:exported="false" />
        <receiver
            android:name="com.crabcheck.alarmlib.BootReceiver"
            android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.BOOT_COMPLETED" />
                <action android:name="android.intent.action.MY_PACKAGE_REPLACED" />
                <action android:name="android.intent.action.TIME_SET" />
                <action android:name="android.intent.action.TIMEZONE_CHANGED" />
            </intent-filter>
        </receiver>
        <service
            android:name="com.crabcheck.alarmlib.AlarmAudioService"
            android:exported="false"
            android:foregroundServiceType="mediaPlayback" />
        <service
            android:name="com.crabcheck.alarmlib.AlarmAccessibilityService"
            android:exported="false"
            android:permission="android.permission.BIND_ACCESSIBILITY_SERVICE">
            <intent-filter>
                <action android:name="android.accessibilityservice.AccessibilityService" />
            </intent-filter>
            <meta-data
                android:name="android.accessibilityservice"
                android:resource="@xml/accessibility_service_config" />
        </service>
        <activity
            android:name="com.crabcheck.alarmlib.LockActivity"
            android:exported="false"
            android:showOnLockScreen="true"
            android:turnScreenOn="true"
            android:launchMode="singleInstance"
            android:screenOrientation="portrait"
            android:theme="@style/Theme.AppCompat.Light.NoActionBar" />
    </application>"#;
            manifest = manifest.replace("    </application>", components);
        } else {
            manifest = manifest.replace(
                "android:theme=\"@style/AppTheme\" />",
                "android:theme=\"@style/Theme.AppCompat.Light.NoActionBar\" />",
            );
            if !manifest.contains("com.crabcheck.alarmlib.AlarmAudioService") {
                let extra_components = r#"        <receiver
            android:name="com.crabcheck.alarmlib.BootReceiver"
            android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.BOOT_COMPLETED" />
                <action android:name="android.intent.action.MY_PACKAGE_REPLACED" />
                <action android:name="android.intent.action.TIME_SET" />
                <action android:name="android.intent.action.TIMEZONE_CHANGED" />
            </intent-filter>
        </receiver>
        <service
            android:name="com.crabcheck.alarmlib.AlarmAudioService"
            android:exported="false"
            android:foregroundServiceType="mediaPlayback" />
        <service
            android:name="com.crabcheck.alarmlib.AlarmAccessibilityService"
            android:exported="false"
            android:permission="android.permission.BIND_ACCESSIBILITY_SERVICE">
            <intent-filter>
                <action android:name="android.accessibilityservice.AccessibilityService" />
            </intent-filter>
            <meta-data
                android:name="android.accessibilityservice"
                android:resource="@xml/accessibility_service_config" />
        </service>
    </application>"#;
                manifest = manifest.replace("    </application>", extra_components);
            }
        }
        if !manifest.contains("android.permission.CAMERA") {
            let cam_perm = "    <uses-permission android:name=\"android.permission.CAMERA\" />\n    <application";
            manifest = manifest.replace("    <application", cam_perm);
        }
        if !manifest.contains("android.permission.RECEIVE_BOOT_COMPLETED") {
            let boot_perms = concat!(
                "    <uses-permission android:name=\"android.permission.RECEIVE_BOOT_COMPLETED\" />\n",
                "    <uses-permission android:name=\"android.permission.FOREGROUND_SERVICE\" />\n",
                "    <uses-permission android:name=\"android.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK\" />\n",
                "    <uses-permission android:name=\"android.permission.VIBRATE\" />\n",
                "    <application"
            );
            manifest = manifest.replace("    <application", boot_perms);
        }
        if !manifest.contains("android.permission.SYSTEM_ALERT_WINDOW") {
            let onboarding_perms = concat!(
                "    <uses-permission android:name=\"android.permission.SYSTEM_ALERT_WINDOW\" />\n",
                "    <uses-permission android:name=\"android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS\" />\n",
                "    <application"
            );
            manifest = manifest.replace("    <application", onboarding_perms);
        }
        manifest = manifest.replace("android:icon=\"@mipmap/ic_launcher\"", "android:icon=\"@drawable/crabcheck_logo\"");
        manifest = manifest.replace("android:roundIcon=\"@mipmap/ic_launcher_round\"", "android:roundIcon=\"@drawable/crabcheck_logo\"");
        fs::write(&manifest_path, manifest)?;
    }

    Ok(())
}
