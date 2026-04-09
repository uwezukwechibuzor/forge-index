//! `forge dev` — development mode with hot reload.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::process::ProcessManager;
use crate::watcher::FileWatcher;

/// Runs the dev command: build, watch, and hot-reload on changes.
pub fn run(entry: PathBuf, port: u16, schema: Option<String>) -> anyhow::Result<()> {
    println!("🔥 forge dev — starting in development mode");

    let mut env_vars = HashMap::new();
    env_vars.insert("FORGE_ENV".to_string(), "dev".to_string());
    env_vars.insert("FORGE_PORT".to_string(), port.to_string());
    if let Some(s) = &schema {
        env_vars.insert("FORGE_SCHEMA".to_string(), s.clone());
    }

    // Determine binary name from the project
    let binary_path = find_binary(&entry)?;

    // Initial build
    println!("Building...");
    if !run_cargo_build(false) {
        println!("❌ Initial build failed — waiting for file changes");
    } else {
        println!("✅ Build successful");
    }

    let mut pm = ProcessManager::new(binary_path.clone(), env_vars.clone());

    // Start the process if build was successful
    if binary_path.exists() {
        if let Err(e) = pm.start() {
            println!("⚠️  Failed to start process: {}", e);
        }
    }

    // Watch for file changes
    let watch_paths = vec![
        PathBuf::from("src"),
        PathBuf::from("Cargo.toml"),
        PathBuf::from(".env"),
    ];

    let watcher = FileWatcher::new(&watch_paths)?;

    loop {
        match watcher.wait_for_change() {
            Some(path) => {
                println!("📝 File changed: {} — rebuilding...", path.display());

                pm.kill()?;

                if run_cargo_build(false) {
                    println!("✅ Build successful — restarting...");
                    pm = ProcessManager::new(binary_path.clone(), env_vars.clone());
                    if let Err(e) = pm.start() {
                        println!("⚠️  Failed to restart: {}", e);
                    } else {
                        println!("🔄 Reloaded successfully");
                    }
                } else {
                    println!("❌ Build failed — waiting for next change");
                }
            }
            None => {
                println!("File watcher stopped");
                break;
            }
        }
    }

    pm.kill()?;
    Ok(())
}

/// Runs `cargo build` and returns whether it succeeded.
fn run_cargo_build(release: bool) -> bool {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build");
    if release {
        cmd.arg("--release");
    }
    match cmd.status() {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

/// Finds the binary path from the project's target directory.
fn find_binary(entry: &std::path::Path) -> anyhow::Result<PathBuf> {
    // Try to find the binary name from Cargo.toml
    let cargo_toml = PathBuf::from("Cargo.toml");
    if cargo_toml.exists() {
        let content = std::fs::read_to_string(&cargo_toml)?;
        // Simple parsing: look for [[bin]] name or package name
        for line in content.lines() {
            if let Some(name) = line.strip_prefix("name = ") {
                let name = name.trim().trim_matches('"');
                return Ok(PathBuf::from(format!("target/debug/{}", name)));
            }
        }
    }

    // Fallback: use the entry filename without extension
    let name = entry
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("forge-indexer");

    Ok(PathBuf::from(format!("target/debug/{}", name)))
}
