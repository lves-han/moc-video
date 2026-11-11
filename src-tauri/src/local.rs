//! Local video library: the files and folders the user added, scanned on demand.
//! Opening re-checks that a file still resolves inside one of those roots.
use crate::{contracts::ApiError, source_dispatch::identity};
use serde_json::{Value, json};
use std::{fs::File, path::{Path, PathBuf}};

const SCAN_LIMIT: usize = 20_000;
const DEPTH_LIMIT: usize = 8;
const VIDEO: [&str; 9] = ["mp4", "m4v", "mov", "mkv", "webm", "avi", "ts", "flv", "wmv"];
// Formats WKWebView decodes directly, plus MKV, which is remuxed to fMP4 HLS on demand.
const PLAYABLE: [&str; 4] = ["mp4", "m4v", "mov", "mkv"];
pub const CONFIG_ID: &str = "0c5e1d2a-3b4c-4d5e-8f6a-7b8c9d0e1f2a";

fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

pub fn file_id(path: &Path) -> String {
    identity("local_file", &[&path.to_string_lossy()])
}

pub fn mime(path: &Path) -> &'static str {
    match extension(path).as_str() {
        "m4v" => "video/x-m4v",
        "mov" => "video/quicktime",
        _ => "video/mp4",
    }
}

pub fn remuxed(path: &Path) -> bool {
    extension(path) == "mkv"
}

pub fn supported(path: &Path) -> bool {
    PLAYABLE.contains(&extension(path).as_str())
}

pub fn roots(settings: &Value) -> Vec<PathBuf> {
    settings["local_paths"].as_array().into_iter().flatten().filter_map(Value::as_str).map(PathBuf::from).collect()
}

fn hidden(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.'))
}

fn walk(dir: &Path, root: &Path, depth: usize, out: &mut Vec<(PathBuf, PathBuf, u64)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if out.len() >= SCAN_LIMIT || hidden(&path) {
            continue;
        }
        // symlink_metadata keeps the scan from following links out of the folder.
        let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
        if meta.is_dir() && depth < DEPTH_LIMIT {
            walk(&path, root, depth + 1, out);
        } else if meta.is_file() && VIDEO.contains(&extension(&path).as_str()) {
            out.push((path, root.to_owned(), meta.len()));
        }
    }
}

/// Scans every root; returns the listing and the id → path table used to open files.
pub fn scan(settings: &Value) -> (Value, Vec<(String, PathBuf)>) {
    let mut found = Vec::new();
    let mut roots_view = Vec::new();
    for root in roots(settings) {
        let meta = std::fs::metadata(&root);
        roots_view.push(json!({"path": root.to_string_lossy(), "available": meta.is_ok()}));
        match meta {
            Ok(meta) if meta.is_dir() => walk(&root, &root, 0, &mut found),
            Ok(meta) if meta.is_file() => found.push((root.clone(), root.clone(), meta.len())),
            _ => {}
        }
    }
    found.truncate(SCAN_LIMIT);
    let mut table = Vec::new();
    let items: Vec<Value> = found.iter().map(|(path, root, size)| {
        let id = file_id(path);
        table.push((id.clone(), path.clone()));
        json!({"id": id, "name": path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            "path": path.to_string_lossy(), "root": root.to_string_lossy(), "size": size, "supported": supported(path)})
    }).collect();
    (json!({"roots": roots_view, "items": items}), table)
}

/// The file only if it still resolves inside a configured root.
pub fn checked(settings: &Value, path: &Path) -> Result<PathBuf, ApiError> {
    let missing = || ApiError::new("SOURCE_NOT_FOUND", "Local file was moved or deleted");
    let real = std::fs::canonicalize(path).map_err(|_| missing())?;
    if !std::fs::metadata(&real).map_err(|_| missing())?.is_file() {
        return Err(missing());
    }
    let inside = roots(settings).iter().filter_map(|root| std::fs::canonicalize(root).ok()).any(|root| real == root || real.starts_with(&root));
    if !inside {
        return Err(ApiError::new("UNAUTHORIZED", "Local file is outside the library"));
    }
    Ok(real)
}

/// Open first, then verify the actual file behind the descriptor. The caller keeps
/// this descriptor for the entire session, including after a path is replaced.
pub fn checked_file(settings: &Value, path: &Path) -> Result<(PathBuf, File), ApiError> {
    let resolved = checked(settings, path)?;
    let file = File::open(&resolved).map_err(|_| ApiError::new("SOURCE_NOT_FOUND", "Local file was moved or deleted"))?;
    #[cfg(target_os = "macos")]
    let actual = {
        use std::os::{fd::AsRawFd, unix::ffi::OsStrExt};
        let mut buffer = [0u8; libc::PATH_MAX as usize];
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETPATH, buffer.as_mut_ptr()) } == -1 {
            return Err(ApiError::new("SOURCE_NOT_FOUND", "Cannot verify opened local file"));
        }
        let end = buffer.iter().position(|b| *b == 0).ok_or_else(|| ApiError::internal("Local file path is too long"))?;
        PathBuf::from(std::ffi::OsStr::from_bytes(&buffer[..end]))
    };
    #[cfg(target_os = "linux")]
    let actual = {
        use std::os::fd::AsRawFd;
        std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))
            .map_err(|_| ApiError::new("SOURCE_NOT_FOUND", "Cannot verify opened local file"))?
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return Err(ApiError::new("CAPABILITY_UNAVAILABLE", "Secure local playback is unavailable on this platform"));
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let actual = checked(settings, &actual)?;
        use std::os::unix::fs::MetadataExt;
        let opened = file.metadata().map_err(|_| ApiError::new("SOURCE_NOT_FOUND", "Cannot verify opened local file"))?;
        let current = std::fs::metadata(&actual).map_err(|_| ApiError::new("SOURCE_NOT_FOUND", "Cannot verify opened local file"))?;
        if opened.dev() != current.dev() || opened.ino() != current.ino() {
            return Err(ApiError::new("UNAUTHORIZED", "Local file changed during authorization"));
        }
        Ok((actual, file))
    }
}

/// The durable identity a local file uses in watch history.
pub fn history_identity(id: &str) -> Value {
    json!({"config_id": CONFIG_ID, "addon_key": identity("local_library", &[]), "namespace": "local",
        "type": "file", "meta_id": id, "canonical_id": id})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_lists_videos_marks_support_and_checks_scope() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().join("Movies");
        std::fs::create_dir_all(root.join("Sub")).unwrap();
        std::fs::write(root.join("a.mp4"), b"a").unwrap();
        std::fs::write(root.join("Sub/b.mkv"), b"bb").unwrap();
        std::fs::write(root.join("notes.txt"), b"x").unwrap();
        std::fs::write(root.join(".hidden.mp4"), b"x").unwrap();
        let outside = folder.path().join("outside.mp4");
        std::fs::write(&outside, b"o").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("link.mp4")).unwrap();
        let settings = json!({"local_paths": [root.to_string_lossy(), folder.path().join("gone").to_string_lossy()]});
        let (view, table) = scan(&settings);
        let names: Vec<_> = view["items"].as_array().unwrap().iter().map(|i| (i["name"].as_str().unwrap().to_owned(), i["supported"].as_bool().unwrap())).collect();
        assert_eq!(names, vec![("b.mkv".to_owned(), true), ("a.mp4".to_owned(), true)], "sorted path order");
        assert_eq!(view["roots"][1]["available"], false);
        assert_eq!(table.len(), 2);
        assert!(checked(&settings, &root.join("a.mp4")).is_ok());
        assert_eq!(checked(&settings, &outside).unwrap_err().code, "UNAUTHORIZED");
        #[cfg(unix)]
        assert_eq!(checked(&settings, &root.join("link.mp4")).unwrap_err().code, "UNAUTHORIZED", "a link cannot escape the library");
        std::fs::remove_file(root.join("a.mp4")).unwrap();
        assert_eq!(checked(&settings, &root.join("a.mp4")).unwrap_err().code, "SOURCE_NOT_FOUND");
    }
}
