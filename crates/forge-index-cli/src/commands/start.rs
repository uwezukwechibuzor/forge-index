//! `forge start` — production mode.

use std::path::PathBuf;
use std::process::Command;

/// Runs the start command: build in release mode, then exec the binary.
pub fn run(entry: PathBuf, port: u16, schema: Option<String>) -> anyhow::Result<()> {
    println!("🚀 forge start — starting in production mode");

    // Build in release mode
    println!("Building (release)...");
    let build_status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .status()?;

    if !build_status.success() {
        anyhow::bail!("Release build failed");
    }

    println!("✅ Build successful — starting...");

    // Find the binary
    let binary_path = find_release_binary(&entry)?;

    // Run the binary with production env vars
    let mut cmd = Command::new(&binary_path);
    cmd.env("FORGE_ENV", "prod");
    cmd.env("FORGE_PORT", port.to_string());
    if let Some(s) = &schema {
        cmd.env("FORGE_SCHEMA", s);
    }

    let status = cmd
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run {}: {}", binary_path.display(), e))?;

    std::process::exit(status.code().unwrap_or(1));
}

/// Finds the release binary path.
fn find_release_binary(entry: &std::path::Path) -> anyhow::Result<PathBuf> {
    let cargo_toml = PathBuf::from("Cargo.toml");
    if cargo_toml.exists() {
        let content = std::fs::read_to_string(&cargo_toml)?;
        for line in content.lines() {
            if let Some(name) = line.strip_prefix("name = ") {
                let name = name.trim().trim_matches('"');
                return Ok(PathBuf::from(format!("target/release/{}", name)));
            }
        }
    }

    let name = entry
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("forge-indexer");

    Ok(PathBuf::from(format!("target/release/{}", name)))
}
