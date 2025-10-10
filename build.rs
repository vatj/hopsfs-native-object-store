use fs_extra::dir::CopyOptions;
use log::info;
use std::env;
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // env_logger::init();
    info!("Starting build script...");

    // Check for system-hdfs feature first
    if env::var("CARGO_FEATURE_SYSTEM_HDFS").is_ok() {
        // Use system hdfs library
        println!("cargo:rustc-link-lib=hdfs");
        if let Ok(lib_path) = env::var("HDFS_LIB_PATH") {
            println!("cargo:rustc-link-search=native={}", lib_path);
        }
        return Ok(());
    }

    // Check for user-provided library path override
    if let Ok(lib_path) = env::var("HDFS_LIB_PATH") {
        println!("cargo:rustc-link-search=native={}", lib_path);
        println!("cargo:rustc-link-lib=hdfs");
        return Ok(());
    }

    extract_tarball()?;
    set_libraries();
    Ok(())
}

fn extract_tarball() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("CARGO_FEATURE_SKIP_DOWNLOAD").is_ok() {
        info!("Downloading dependencies skipped.");
        return Ok(());
    }

    let base_url = env::var("HOPS_LIB_BASE_URL")
        .expect("HOPS_LIB_BASE_URL environment variable is not set");

    let filename = find_hops_lib_filename();

    let lib_url = format!("{}/{}", base_url, filename);
    let tarball_path = Path::new(&filename);

    info!("Downloading tarball from {}", lib_url);

    let lib_username = env::var("HOPS_LIB_USERNAME").unwrap_or_default();
    let lib_password = env::var("HOPS_LIB_PASSWORD").unwrap_or_default();

    let client = reqwest::blocking::Client::new();
    let mut request_builder = client.get(&lib_url);
    if !lib_username.is_empty() && !lib_password.is_empty() {
        request_builder = request_builder.basic_auth(lib_username, Some(lib_password));
    }
    let mut response = request_builder.send()?;
    if !response.status().is_success() {
        return Err(format!("Failed to download file: HTTP {}", response.status()).into());
    }
    let mut tarball_file = File::create(tarball_path)?;
    io::copy(&mut response, &mut tarball_file)?;
    info!("Downloaded tarball to {:?}", tarball_path);

    let extract_dir = PathBuf::from("temp_extracted");
    if extract_dir.exists() {
        fs::remove_dir_all(&extract_dir)?;
    }
    fs::create_dir(&extract_dir)?;

    let tarball_file = File::open(tarball_path)?;
    let decompressor = flate2::read::GzDecoder::new(tarball_file);
    let mut archive = tar::Archive::new(decompressor);
    archive.unpack(&extract_dir)?;
    info!("Extracted tarball to {:?}", extract_dir);

    let subdirs: Vec<PathBuf> = fs::read_dir(&extract_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    if subdirs.len() != 1 {
        return Err("Expected exactly one subdirectory in the extracted tarball".into());
    }

    let extracted_folder = &subdirs[0];
    let search_dir = extracted_folder.join("lib/native/libhdfs-golang");
    let mut options = CopyOptions::default();
    options.overwrite = true;
    options.content_only = true;

    let lib_dir = Path::new("lib");
    if !lib_dir.exists() {
        fs::create_dir(lib_dir)?;
    }
    else {
        fs::remove_dir_all(lib_dir)?;
    }

    fs_extra::dir::copy(&search_dir, lib_dir, &options)?;
    info!(
        "Copied library and header files to directory: {:?}",
        lib_dir
    );

    fs::remove_dir_all(&extract_dir)?;
    fs::remove_file(tarball_path)?;
    info!("Cleaned up temporary files");
    Ok(())
}

fn find_hops_lib_filename() -> String {
    let version_content = fs::read_to_string("HOPS_VERSION")
        .expect("Failed to read HOPS_VERSION file");
    let version = version_content.trim();
    format!("hops-{}.tgz", version)

}

#[cfg(target_os = "macos")]
fn set_libraries() {
    create_symlinks("macos".to_string(), false);
    println!("cargo:rustc-link-search=native=.");
    println!("cargo:rustc-link-lib=static=hdfs");
    println!("cargo:rustc-link-lib=framework=Security");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
}

#[cfg(target_os = "linux")]
fn set_libraries() {
    create_symlinks("linux".to_string(), true);
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    
    // Add library search paths
    println!("cargo:rustc-link-search=native={}", manifest_dir);
    println!("cargo:rustc-link-search=native={}/lib", manifest_dir);
    
    // Check for the actual library file first, then try different linking strategies
    let lib_dir = Path::new(&manifest_dir).join("lib");
    let direct_a_path = lib_dir.join("libhdfs-golang-3.2.0.18-EE-SNAPSHOT-linux-amd64.a");
    let direct_so_path = lib_dir.join("libhdfs-golang-3.2.0.18-EE-SNAPSHOT-linux-amd64.so");
    let symlink_so_path = Path::new(&manifest_dir).join("libhdfs.so");
    
    // Prefer static linking to avoid shared library issues in git dependencies
    if direct_a_path.exists() {
        // Use static linking - most reliable for git dependencies
        println!("cargo:rustc-link-lib=static=hdfs-golang-3.2.0.18-EE-SNAPSHOT-linux-amd64");
    } else if symlink_so_path.exists() {
        // Try using the symlink
        println!("cargo:rustc-link-lib=hdfs");
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/lib");
    } else if direct_so_path.exists() {
        // Use the direct shared library path
        println!("cargo:rustc-link-lib=dylib=hdfs-golang-3.2.0.18-EE-SNAPSHOT-linux-amd64");
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
        println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/lib");
    } else {
        // Final fallback
        println!("cargo:rustc-link-lib=static=hdfs");
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn main() {
    panic!("Unsupported target OS: HopsFS object store only supports macOS and Linux.");
}

fn create_symlinks(target_os: String, shared: bool) {
    let lib_dir = Path::new("lib");

    // Only create symlinks, don't emit any linking directives here
    // Create symlinks for all supported architectures to ensure compatibility 
    // when this crate is used as a dependency
    create_symlink_for_platform(lib_dir, "linux-amd64", ".so", "libhdfs.so");
    create_symlink_for_platform(lib_dir, "linux-amd64", ".a", "libhdfs-static.a");
    create_symlink_for_platform(lib_dir, "darwin-10.12-arm64", ".a", "libhdfs-darwin.a");

    // Create the primary symlink for the current target
    let filter = match target_os.as_str() {
        "linux" => "linux-amd64",
        "macos" => "arm64",
        other => panic!("Unsupported target OS: {}", other),
    };

    let lib_ext = if shared {
        match target_os.as_str() {
            "linux" => ".so",
            "macos" => ".dylib",
            other => panic!("Unsupported target OS: {}", other),
        }
    } else {
        ".a"
    };

    let mut lib_file = None;
    let mut header_file = None;
    for entry in fs::read_dir(lib_dir).expect("Could not read lib directory") {
        let entry = entry.expect("Error reading directory entry");
        let file_name = entry.file_name().into_string().expect("Invalid file name");

        if file_name.ends_with(lib_ext) && file_name.contains(filter) {
            lib_file = Some(entry.path());
        } else if file_name.ends_with(".h") && file_name.contains(filter) {
            header_file = Some(entry.path());
        }
    }

    if let (Some(lib_file), Some(header_file)) = (lib_file, header_file) {
        let symlink_lib_str = format!("libhdfs{}", lib_ext);
        let symlink_lib = Path::new(&symlink_lib_str);
        let symlink_header = Path::new("libhdfs.h");

        if symlink_lib.exists() {
            let _ = fs::remove_file(symlink_lib);
        }
        if symlink_header.exists() {
            let _ = fs::remove_file(symlink_header);
        }

        let _ = symlink(&lib_file, symlink_lib);
        let _ = symlink(&header_file, symlink_header);
    }
}

fn create_symlink_for_platform(lib_dir: &Path, arch_filter: &str, lib_ext: &str, symlink_name: &str) {
    let mut lib_file = None;
    for entry in fs::read_dir(lib_dir).expect("Could not read lib directory") {
        let entry = entry.expect("Error reading directory entry");
        let file_name = entry.file_name().into_string().expect("Invalid file name");

        if file_name.ends_with(lib_ext) && file_name.contains(arch_filter) {
            lib_file = Some(entry.path());
            break;
        }
    }

    if let Some(lib_file) = lib_file {
        let symlink_lib = Path::new(symlink_name);
        
        if symlink_lib.exists() {
            let _ = fs::remove_file(symlink_lib);
        }
        let _ = symlink(&lib_file, symlink_lib);
    }
}
