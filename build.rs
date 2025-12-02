use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // tonic_prost_build requires the vendored protoc binary
    let protoc_path =
        protoc_bin_vendored::protoc_bin_path().expect("Failed to find vendored protoc");
    unsafe { std::env::set_var("PROTOC", &protoc_path) };

    // Compile protobuf files together so imports work correctly
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir(&out_dir)
        .compile_protos(
            &["proto/market_data.proto", "proto/control.proto"],
            &["proto"],
        )?;

    // Tell cargo to recompile if proto files change
    println!("cargo:rerun-if-changed=proto/market_data.proto");
    println!("cargo:rerun-if-changed=proto/control.proto");
    println!("cargo:rerun-if-changed=build.rs");

    // Add build metadata for version information
    add_build_metadata()?;

    Ok(())
}

fn add_build_metadata() -> Result<(), Box<dyn std::error::Error>> {
    // Build timestamp
    let build_timestamp = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    println!("cargo:rustc-env=VERGEN_BUILD_TIMESTAMP={build_timestamp}");

    // Git SHA (if available)
    let git_sha = get_git_sha().unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=VERGEN_GIT_SHA={git_sha}");

    // Rust version
    let rustc_version = get_rustc_version().unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=VERGEN_RUSTC_SEMVER={rustc_version}");

    // Target triple
    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=VERGEN_CARGO_TARGET_TRIPLE={target}");

    Ok(())
}

fn get_git_sha() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn get_rustc_version() -> Option<String> {
    let output = Command::new("rustc").args(["--version"]).output().ok()?;

    if output.status.success() {
        let version_str = String::from_utf8_lossy(&output.stdout);
        // Extract just the version number (e.g., "1.70.0" from "rustc 1.70.0 (90c541806 2023-05-31)")
        version_str.split_whitespace().nth(1).map(|v| v.to_string())
    } else {
        None
    }
}
