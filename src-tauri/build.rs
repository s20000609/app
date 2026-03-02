use std::env;
use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

const ORT_VERSION: &str = "1.22.0";

fn main() {
    println!("cargo:rerun-if-env-changed=ORT_LIB_LOCATION");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "android" {
        println!("cargo:warning=Detected Android build, checking ONNX Runtime libraries...");
        setup_android_libs().expect("Failed to setup Android libraries");
    } else if target_os == "ios" {
        println!("cargo:warning=Detected iOS build.");
        if std::env::var("ORT_LIB_LOCATION").is_err() {
            println!(
                "cargo:warning=ORT_LIB_LOCATION is not set. iOS builds require a CoreML-enabled ONNX Runtime library location."
            );
        }
    } else if target_os == "macos" {
        println!("cargo:warning=Detected macOS build, preparing ONNX Runtime library...");
        setup_macos_libs().expect("Failed to setup macOS ONNX Runtime library");
    } else {
        println!(
            "cargo:warning=Detected Desktop build, skipping ONNX Runtime download (runtime fetch)."
        );
    }

    tauri_build::build();
}

fn setup_android_libs() -> anyhow::Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let resource_dir = manifest_dir.join("onnxruntime");
    if !resource_dir.exists() {
        fs::create_dir_all(&resource_dir)?;
        println!(
            "cargo:warning=Created ONNX Runtime resource dir at {:?} for Android build",
            resource_dir
        );
    }
    let jni_libs_path = PathBuf::from("gen/android/app/src/main/jniLibs");

    let targets = vec![
        ("arm64-v8a", "jni/arm64-v8a/libonnxruntime.so"),
        ("x86_64", "jni/x86_64/libonnxruntime.so"),
    ];

    let mut missing = false;
    for (arch, _) in &targets {
        let lib_path = jni_libs_path.join(arch).join("libonnxruntime.so");
        if !lib_path.exists() {
            missing = true;
            break;
        }
    }

    if !missing {
        println!("cargo:warning=ONNX Runtime libs already present.");
        return Ok(());
    }

    println!(
        "cargo:warning=Downloading ONNX Runtime Android v{}...",
        ORT_VERSION
    );
    let url = format!(
        "https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android/{0}/onnxruntime-android-{0}.aar",
        ORT_VERSION
    );

    let response = reqwest::blocking::get(&url)?.bytes()?;
    let reader = Cursor::new(response);
    let mut zip = zip::ZipArchive::new(reader)?;

    for (arch, internal_path) in targets {
        let dest_dir = jni_libs_path.join(arch);
        fs::create_dir_all(&dest_dir)?;

        let dest_file = dest_dir.join("libonnxruntime.so");

        match zip.by_name(internal_path) {
            Ok(mut file) => {
                let mut outfile = fs::File::create(&dest_file)?;
                io::copy(&mut file, &mut outfile)?;
                println!("cargo:warning=Extracted: {:?}", dest_file);
            }
            Err(_) => {
                println!(
                    "cargo:warning=Could not find {} in AAR, skipping...",
                    internal_path
                );
            }
        }
    }

    Ok(())
}

fn setup_macos_libs() -> anyhow::Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let resource_dir = manifest_dir.join("onnxruntime");
    fs::create_dir_all(&resource_dir)?;

    if let Ok(path) = env::var("ORT_LIB_LOCATION") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            copy_macos_dylibs_from_dir(Path::new(trimmed), &resource_dir)?;
            println!(
                "cargo:warning=ORT_LIB_LOCATION is set for macOS build ({}); copied ONNX Runtime dylibs into {:?}.",
                trimmed, resource_dir
            );
            return Ok(());
        }
    }

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let archive_arch = match target_arch.as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        _ => {
            println!(
                "cargo:warning=Unsupported macOS architecture '{}' for bundled ONNX Runtime; runtime fetch fallback will be used.",
                target_arch
            );
            return Ok(());
        }
    };

    let dylib_path = resource_dir.join("libonnxruntime.dylib");
    let shared_path = resource_dir.join("libonnxruntime_providers_shared.dylib");
    if dylib_path.exists() && shared_path.exists() {
        println!(
            "cargo:warning=macOS ONNX Runtime already present at {:?}",
            dylib_path
        );
        return Ok(());
    }

    let archive_url = format!(
        "https://github.com/microsoft/onnxruntime/releases/download/v{0}/onnxruntime-osx-{1}-{0}.tgz",
        ORT_VERSION, archive_arch
    );
    let lib_dir_in_archive = format!("onnxruntime-osx-{}-{}/lib/", archive_arch, ORT_VERSION);

    println!(
        "cargo:warning=Downloading ONNX Runtime macOS v{} ({})...",
        ORT_VERSION, archive_arch
    );
    let response = reqwest::blocking::get(&archive_url)?.bytes()?;
    extract_tgz_dylibs_from_dir(&response, &lib_dir_in_archive, &resource_dir)?;
    if dylib_path.exists() {
        println!("cargo:warning=Extracted: {:?}", dylib_path);
    }

    Ok(())
}

fn copy_macos_dylibs_from_dir(src_dir: &Path, dest_dir: &Path) -> anyhow::Result<()> {
    if !src_dir.exists() {
        anyhow::bail!("ORT_LIB_LOCATION does not exist: {}", src_dir.display());
    }
    if !src_dir.is_dir() {
        anyhow::bail!("ORT_LIB_LOCATION is not a directory: {}", src_dir.display());
    }

    let mut copied_count = 0usize;
    let mut has_main_dylib = false;
    let mut has_shared_provider = false;

    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("dylib") {
            continue;
        }

        let Some(file_name) = path.file_name() else {
            continue;
        };
        let dest_path = dest_dir.join(file_name);
        fs::copy(&path, &dest_path)?;
        copied_count += 1;

        match file_name.to_string_lossy().as_ref() {
            "libonnxruntime.dylib" => has_main_dylib = true,
            "libonnxruntime_providers_shared.dylib" => has_shared_provider = true,
            _ => {}
        }
    }

    if copied_count == 0 {
        anyhow::bail!(
            "No .dylib files found in ORT_LIB_LOCATION: {}",
            src_dir.display()
        );
    }

    if !has_main_dylib {
        anyhow::bail!(
            "ORT_LIB_LOCATION is missing libonnxruntime.dylib: {}",
            src_dir.display()
        );
    }

    if !has_shared_provider {
        anyhow::bail!(
            "ORT_LIB_LOCATION is missing libonnxruntime_providers_shared.dylib: {}",
            src_dir.display()
        );
    }

    Ok(())
}

fn extract_tgz_dylibs_from_dir(
    bytes: &[u8],
    entry_dir: &str,
    dest_dir: &Path,
) -> anyhow::Result<()> {
    let reader = Cursor::new(bytes);
    let tar = flate2::read::GzDecoder::new(reader);
    let mut archive = tar::Archive::new(tar);
    let mut extracted_count = 0usize;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().replace('\\', "/");
        if !path.starts_with(entry_dir) || !path.ends_with(".dylib") {
            continue;
        }
        let Some(filename) = Path::new(&path).file_name() else {
            continue;
        };
        fs::create_dir_all(dest_dir)?;
        let out_path = dest_dir.join(filename);
        let mut outfile = fs::File::create(&out_path)?;
        io::copy(&mut entry, &mut outfile)?;
        extracted_count += 1;
    }

    if extracted_count == 0 {
        anyhow::bail!(
            "No .dylib entries found under '{}' in ONNX Runtime archive",
            entry_dir
        );
    }

    Ok(())
}
