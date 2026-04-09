//! File watcher for hot reload using the `notify` crate with debouncing.

use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Debounce interval for file change events.
const DEBOUNCE_MS: u64 = 500;

/// Watches files and directories for changes, with debouncing.
pub struct FileWatcher {
    /// The debouncer handle (must be kept alive).
    _debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
    /// Receiver for debounced events.
    rx: mpsc::Receiver<Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>>,
}

impl FileWatcher {
    /// Creates a new file watcher for the given paths.
    pub fn new(paths: &[PathBuf]) -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel();

        let mut debouncer = new_debouncer(Duration::from_millis(DEBOUNCE_MS), tx)
            .map_err(|e| anyhow::anyhow!("Failed to create file watcher: {}", e))?;

        for path in paths {
            if path.exists() {
                debouncer
                    .watcher()
                    .watch(path, notify::RecursiveMode::Recursive)
                    .map_err(|e| anyhow::anyhow!("Failed to watch {}: {}", path.display(), e))?;
            }
        }

        Ok(Self {
            _debouncer: debouncer,
            rx,
        })
    }

    /// Blocks until a file change is detected. Returns the first changed path.
    pub fn wait_for_change(&self) -> Option<PathBuf> {
        loop {
            match self.rx.recv() {
                Ok(Ok(events)) => {
                    for event in &events {
                        if event.kind == DebouncedEventKind::Any {
                            let path = &event.path;
                            if !should_ignore(path) {
                                return Some(path.clone());
                            }
                        }
                    }
                }
                Ok(Err(_)) | Err(_) => return None,
            }
        }
    }

    /// Non-blocking: drains all pending changed paths.
    pub fn changed_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        while let Ok(Ok(events)) = self.rx.try_recv() {
            for event in events {
                if event.kind == DebouncedEventKind::Any && !should_ignore(&event.path) {
                    paths.push(event.path);
                }
            }
        }
        paths
    }
}

/// Returns `true` if the path should be ignored by the watcher.
fn should_ignore(path: &Path) -> bool {
    let path_str = path.to_string_lossy();

    // Ignore target/ directory
    if path_str.contains("/target/") || path_str.contains("\\target\\") {
        return true;
    }

    // Ignore lock files
    if path_str.ends_with(".lock") {
        return true;
    }

    // Ignore hidden files (starting with .)
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.starts_with('.') {
            return true;
        }
    }

    false
}
