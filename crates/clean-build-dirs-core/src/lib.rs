// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Shared scanning logic for `clean-build-dirs` and `clean-build-dirs-async`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

pub const DIRS_TO_REMOVE: &[&str] = &["target", "node_modules"];

/// Walk `root` and return every directory whose name is in [`DIRS_TO_REMOVE`].
/// Matched directories are not descended into.
pub fn find_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut it = WalkDir::new(root).follow_links(false).into_iter();
    loop {
        let entry = match it.next() {
            None => break,
            Some(Err(_)) => continue,
            Some(Ok(e)) => e,
        };
        if entry.file_type().is_dir()
            && DIRS_TO_REMOVE
                .iter()
                .any(|&s| OsStr::new(s) == entry.file_name())
        {
            out.push(entry.into_path());
            it.skip_current_dir();
        }
    }
    out
}
