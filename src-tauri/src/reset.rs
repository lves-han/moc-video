//! Reset only the current application identifier's storage, before opening any stores.
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
use tauri::{Manager, Runtime};

const MARKER: &[u8] = b"moc-video-reset-v1\n";

/// Validate before scheduling deletion: macOS relaunch can reject symlinked
/// executables, while a Linux AppImage may have been moved or made non-executable.
pub(crate) fn check_relaunch_path(path: &Path) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::other("App restart target is not a file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "App restart target is not executable",
            ));
        }
    }
    Ok(())
}

#[cfg(any(target_os = "macos", test))]
fn macos_webview_roots(home: &Path, temporary: &Path, identifier: &str) -> [PathBuf; 3] {
    let library = home.join("Library");
    [
        library.join("WebKit").join(identifier),
        library.join("HTTPStorages").join(identifier),
        temporary.join(identifier),
    ]
}

pub(crate) struct ResetPlan {
    roots: Vec<PathBuf>,
    marker: PathBuf,
    instance: PathBuf,
}

pub(crate) struct InstanceGuard {
    _file: File,
    path: PathBuf,
}

pub(crate) struct RecoveryState;

impl ResetPlan {
    fn new(identifier: &str, data: &Path, mut roots: Vec<PathBuf>) -> io::Result<Self> {
        if identifier.is_empty()
            || !identifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
            || matches!(identifier, "." | "..")
        {
            return Err(io::Error::other("Invalid reset application identifier"));
        }
        roots.push(data.to_owned());
        for root in &roots {
            // Never accept a caller-provided path, a shared parent or a traversal.
            if !root.is_absolute()
                || root.file_name() != Some(identifier.as_ref())
                || root
                    .components()
                    .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
            {
                return Err(io::Error::other(
                    "Reset path is outside application storage",
                ));
            }
        }
        roots.sort();
        roots.dedup();
        let parent = data
            .parent()
            .ok_or_else(|| io::Error::other("Reset storage has no parent"))?;
        Ok(Self {
            roots,
            marker: parent.join(format!(".{identifier}.reset-pending")),
            instance: parent.join(format!(".{identifier}.instance.lock")),
        })
    }

    pub(crate) fn for_app<R: Runtime>(app: &impl Manager<R>) -> Result<Self, String> {
        let paths = app.path();
        let identifier = &app.app_handle().config().identifier;
        let data = paths.app_data_dir().map_err(|e| e.to_string())?;
        let roots = vec![
            paths.app_local_data_dir().map_err(|e| e.to_string())?,
            paths.app_config_dir().map_err(|e| e.to_string())?,
            paths.app_cache_dir().map_err(|e| e.to_string())?,
            std::env::temp_dir().join(identifier),
        ];
        let logs = paths.app_log_dir().map_err(|e| e.to_string())?;
        Self::from_storage_paths(identifier, &data, roots, logs, {
            #[cfg(target_os = "macos")]
            {
                Some(macos_webview_roots(
                    &paths.home_dir().map_err(|e| e.to_string())?,
                    &std::env::temp_dir(),
                    identifier,
                ))
            }
            #[cfg(not(target_os = "macos"))]
            {
                None
            }
        })
        .map_err(|e| e.to_string())
    }

    fn from_storage_paths(
        identifier: &str,
        data: &Path,
        mut roots: Vec<PathBuf>,
        logs: PathBuf,
        macos_webview: Option<[PathBuf; 3]>,
    ) -> io::Result<Self> {
        // Linux/XDG and Windows logs are nested; macOS has Library/Logs/id.
        if !logs.starts_with(data) && !roots.iter().any(|root| logs.starts_with(root)) {
            roots.push(logs);
        }
        if let Some(webview) = macos_webview {
            roots.extend(webview);
        }
        Self::new(identifier, data, roots)
    }

    fn validate_roots(&self) -> io::Result<()> {
        for root in &self.roots {
            match fs::symlink_metadata(root) {
                Ok(info) => {
                    #[cfg(windows)]
                    let redirected = {
                        use std::os::windows::fs::MetadataExt;
                        info.file_attributes() & 0x400 != 0 // FILE_ATTRIBUTE_REPARSE_POINT
                    };
                    #[cfg(not(windows))]
                    let redirected = info.file_type().is_symlink();
                    if redirected {
                        return Err(io::Error::other("Reset storage is redirected"));
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// Keep this file locked until the old process exits. The new process waits
    /// for this lock before deleting data, so late writes cannot restore old state.
    fn request(&self) -> io::Result<File> {
        self.validate_roots()?;
        let parent = self.marker.parent().expect("Validated reset marker parent");
        fs::create_dir_all(parent)?;
        let mut marker = tempfile::NamedTempFile::new_in(parent)?;
        marker.write_all(MARKER)?;
        marker.as_file().sync_all()?;
        marker.as_file().try_lock().map_err(io::Error::other)?;
        marker.persist_noclobber(&self.marker).map_err(|e| e.error)
    }

    pub(crate) fn finish_pending(&self) -> io::Result<bool> {
        self.finish_with_timeout(Duration::from_secs(15))
    }

    /// Held for the entire process, before opening Host or a persistent WebView.
    pub(crate) fn acquire_instance(&self, timeout: Duration) -> io::Result<InstanceGuard> {
        fs::create_dir_all(self.instance.parent().expect("Instance lock parent"))?;
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.instance)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        let deadline = Instant::now() + timeout;
        loop {
            match file.try_lock() {
                Ok(()) => {
                    return Ok(InstanceGuard {
                        _file: file,
                        path: self.instance.clone(),
                    });
                }
                Err(fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(25))
                }
                Err(fs::TryLockError::WouldBlock) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "Another app instance owns this profile",
                    ));
                }
                Err(fs::TryLockError::Error(error)) => return Err(error),
            }
        }
    }

    fn require_instance(&self, guard: &InstanceGuard) -> io::Result<()> {
        if guard.path != self.instance {
            return Err(io::Error::other("Wrong application instance lock"));
        }
        Ok(())
    }

    pub(crate) fn schedule(&self, guard: &InstanceGuard) -> io::Result<File> {
        self.require_instance(guard)?;
        self.request()
    }

    pub(crate) fn finish_startup(&self, guard: &InstanceGuard) -> io::Result<bool> {
        self.require_instance(guard)?;
        self.finish_pending()
    }

    /// Cancels further deletion. Files already deleted stay deleted.
    pub(crate) fn cancel_pending(&self, guard: &InstanceGuard) -> io::Result<()> {
        self.require_instance(guard)?;
        let marker = match fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.marker)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        marker.try_lock().map_err(io::Error::other)?;
        fs::remove_file(&self.marker)
    }

    pub(crate) fn recovery_profile(&self) -> PathBuf {
        let identifier = self
            .instance
            .file_name()
            .expect("Instance filename")
            .to_string_lossy();
        let identifier = identifier
            .strip_prefix('.')
            .unwrap()
            .strip_suffix(".instance.lock")
            .unwrap();
        std::env::temp_dir()
            .join(identifier)
            .join(format!("recovery-{}", uuid::Uuid::new_v4()))
    }

    fn finish_with_timeout(&self, timeout: Duration) -> io::Result<bool> {
        let mut marker = match fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.marker)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        let deadline = Instant::now() + timeout;
        loop {
            match marker.try_lock() {
                Ok(()) => break,
                Err(_) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(error) => return Err(io::Error::other(error)),
            }
        }
        // Another simultaneous startup may have finished while this one waited.
        if !self.marker.exists() {
            return Ok(false);
        }
        let mut bytes = Vec::new();
        (&mut marker).take(64).read_to_end(&mut bytes)?;
        if bytes != MARKER {
            return Err(io::Error::other("Invalid reset marker"));
        }
        self.validate_roots()?; // Validate every target before deleting any target.
        for root in &self.roots {
            loop {
                let result = match fs::symlink_metadata(root) {
                    Ok(info) if info.is_dir() => fs::remove_dir_all(root),
                    Ok(_) => fs::remove_file(root),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => break,
                    Err(error) => Err(error),
                };
                match result {
                    Ok(()) => break,
                    Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
                    Err(error) => return Err(error),
                }
            }
        }
        fs::remove_file(&self.marker)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "app.moc.video.reset-test";
    fn plan(temp: &Path) -> ResetPlan {
        let data = temp.join("data").join(ID);
        ResetPlan::new(
            ID,
            &data,
            vec![
                data.clone(),
                temp.join("local").join(ID),
                temp.join("cache").join(ID),
            ],
        )
        .unwrap()
    }

    #[test]
    fn lifetime_instance_lock_blocks_second_writer_and_releases_after_exit() {
        let temp = tempfile::tempdir().unwrap();
        let plan = plan(temp.path());
        let first = plan.acquire_instance(Duration::ZERO).unwrap();
        assert_eq!(
            plan.acquire_instance(Duration::ZERO).err().unwrap().kind(),
            io::ErrorKind::WouldBlock
        );
        let other = ResetPlan::new("app.other", &temp.path().join("app.other"), vec![]).unwrap();
        assert!(other.schedule(&first).is_err());
        drop(first);
        let next = plan.acquire_instance(Duration::ZERO).unwrap();
        assert!(!plan.finish_startup(&next).unwrap());
    }

    #[test]
    fn recovery_can_cancel_corrupt_pending_reset_without_deleting_more_data() {
        let temp = tempfile::tempdir().unwrap();
        let plan = plan(temp.path());
        let guard = plan.acquire_instance(Duration::ZERO).unwrap();
        fs::create_dir_all(&plan.roots[0]).unwrap();
        let keep = plan.roots[0].join("remaining-settings.json");
        fs::write(&keep, b"remaining data").unwrap();
        drop(plan.schedule(&guard).unwrap());
        fs::write(&plan.marker, b"damaged marker").unwrap();
        assert!(plan.finish_startup(&guard).is_err());
        plan.cancel_pending(&guard).unwrap();
        assert!(!plan.finish_startup(&guard).unwrap());
        assert_eq!(fs::read(keep).unwrap(), b"remaining data");
    }

    #[test]
    fn reset_macos_layout_includes_webkit_http_storage_logs_and_transient_media() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("用户 Home");
        let library = home.join("Library");
        let data = library.join("Application Support").join(ID);
        let temporary = temp.path().join("mac temporary");
        let webview = macos_webview_roots(&home, &temporary, ID);
        let plan = ResetPlan::from_storage_paths(
            ID,
            &data,
            vec![data.clone(), data.clone(), library.join("Caches").join(ID)],
            library.join("Logs").join(ID),
            Some(webview.clone()),
        )
        .unwrap();
        assert_eq!(plan.roots.len(), 6);
        for root in webview {
            assert!(plan.roots.contains(&root));
        }
        let unrelated = library.join("WebKit/com.example.other");
        fs::create_dir_all(&unrelated).unwrap();
        fs::write(unrelated.join("keep"), b"other app").unwrap();
        for root in &plan.roots {
            fs::create_dir_all(root).unwrap();
            fs::write(root.join("cached-data"), b"old").unwrap();
        }
        drop(plan.request().unwrap());
        assert!(plan.finish_pending().unwrap());
        assert!(plan.roots.iter().all(|root| !root.exists()));
        assert_eq!(fs::read(unrelated.join("keep")).unwrap(), b"other app");
        assert!(library.exists());
    }

    #[test]
    fn reset_linux_layout_uses_resolved_xdg_roots_and_deduplicates_nested_logs() {
        let temp = tempfile::tempdir().unwrap();
        let xdg = temp.path().join("custom XDG 用户");
        let data = xdg.join("data").join(ID);
        let config = xdg.join("config").join(ID);
        let cache = xdg.join("cache").join(ID);
        let scratch = temp.path().join("temporary").join(ID);
        let plan = ResetPlan::from_storage_paths(
            ID,
            &data,
            vec![data.clone(), config.clone(), cache.clone(), scratch],
            data.join("logs"),
            None,
        )
        .unwrap();
        assert_eq!(plan.roots.len(), 4);
        for root in &plan.roots {
            fs::create_dir_all(root.join("IndexedDB/fonts")).unwrap();
            fs::write(root.join("cookies"), b"old browser data").unwrap();
        }
        fs::create_dir_all(data.join("logs")).unwrap();
        let other = xdg.join("data/com.example.other");
        fs::create_dir_all(&other).unwrap();
        drop(plan.request().unwrap());
        assert!(plan.finish_pending().unwrap());
        assert!(plan.roots.iter().all(|root| !root.exists()));
        assert!(other.exists());
        assert!(xdg.join("config").exists());
    }

    #[test]
    fn reset_relaunch_check_rejects_missing_files_directories_and_unix_non_executables() {
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("Moc Video.AppImage");
        assert!(check_relaunch_path(&binary).is_err());
        assert!(check_relaunch_path(temp.path()).is_err());
        fs::write(&binary, b"test executable").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&binary, fs::Permissions::from_mode(0o600)).unwrap();
            assert!(check_relaunch_path(&binary).is_err());
            fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert!(check_relaunch_path(&binary).is_ok());
    }

    #[test]
    fn reset_waits_for_the_old_process_lock_before_removing_storage() {
        let temp = tempfile::tempdir().unwrap();
        let plan = plan(temp.path());
        for root in &plan.roots {
            fs::create_dir_all(root).unwrap();
        }
        let ready = temp.path().join("worker-ready");
        let release = temp.path().join("worker-release");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "reset::tests::reset_cross_process_worker",
                "--ignored",
                "--test-threads=1",
            ])
            .env("MOC_RESET_WORKER_ROOT", temp.path())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.exists() && Instant::now() < deadline && child.try_wait().unwrap().is_none() {
            thread::sleep(Duration::from_millis(20));
        }
        let started = ready.exists();
        let blocked = started && plan.acquire_instance(Duration::from_millis(60)).is_err();
        let untouched = plan.roots.iter().all(|root| root.exists());
        fs::write(release, b"exit").unwrap();
        let status = child.wait().unwrap();
        assert!(started && status.success(), "reset lock worker failed");
        assert!(
            blocked && untouched,
            "storage was touched before the old process exited"
        );
        let instance = plan.acquire_instance(Duration::from_secs(1)).unwrap();
        assert!(plan.finish_startup(&instance).unwrap());
        assert!(plan.roots.iter().all(|root| !root.exists()));
    }

    #[test]
    #[ignore = "subprocess-only reset lock helper"]
    fn reset_cross_process_worker() {
        let temp =
            PathBuf::from(std::env::var_os("MOC_RESET_WORKER_ROOT").expect("reset worker root"));
        let plan = plan(&temp);
        let _instance = plan.acquire_instance(Duration::from_secs(1)).unwrap();
        let _lock = plan.schedule(&_instance).unwrap();
        fs::write(temp.join("worker-ready"), b"locked").unwrap();
        let release = temp.join("worker-release");
        let deadline = Instant::now() + Duration::from_secs(15);
        while !release.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(release.exists(), "reset worker release timed out");
    }

    #[test]
    fn reset_clears_all_owned_storage_and_reseeds_first_launch() {
        let temp = tempfile::tempdir().unwrap();
        let plan = plan(temp.path());
        for root in &plan.roots {
            fs::create_dir_all(root.join("nested")).unwrap();
            fs::write(root.join("nested/font-and-cache"), b"cached").unwrap();
        }
        let data = temp.path().join("data").join(ID);
        let host = crate::host::Host::open(data.join("settings.json")).unwrap();
        let mut settings: serde_json::Value =
            serde_json::from_slice(&fs::read(data.join("settings.json")).unwrap()).unwrap();
        settings["source_configs"] = serde_json::json!([]);
        settings["font_family"] = serde_json::json!("noto-serif-sc");
        crate::settings::SettingsFile {
            path: data.join("settings.json"),
        }
        .save(&settings)
        .unwrap();
        drop(host);
        fs::write(data.join("moc-video.db"), b"old-history-and-watchlist").unwrap();
        fs::write(temp.path().join("unrelated"), b"keep").unwrap();
        assert!(!plan.finish_pending().unwrap());
        let lock = plan.request().unwrap();
        assert!(plan.finish_with_timeout(Duration::from_millis(60)).is_err());
        assert!(data.join("settings.json").exists());
        drop(lock);
        assert!(plan.finish_pending().unwrap());
        assert!(plan.roots.iter().all(|root| !root.exists()));
        assert_eq!(fs::read(temp.path().join("unrelated")).unwrap(), b"keep");
        let _fresh = crate::host::Host::open(data.join("settings.json")).unwrap();
        let fresh: serde_json::Value =
            serde_json::from_slice(&fs::read(data.join("settings.json")).unwrap()).unwrap();
        assert_ne!(fresh["font_family"], "noto-serif-sc");
        assert!(!fresh["source_configs"].as_array().unwrap().is_empty());
        assert!(!data.join("moc-video.db").exists());
        assert!(!plan.finish_pending().unwrap());
    }

    #[test]
    fn reset_rejects_shared_parents_traversal_and_corrupt_markers() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join(ID);
        assert!(ResetPlan::new(ID, &data, vec![temp.path().to_owned()]).is_err());
        assert!(ResetPlan::new(ID, &data, vec![temp.path().join("x/../").join(ID)]).is_err());
        assert!(ResetPlan::new("../other", &data, vec![]).is_err());
        let plan = plan(temp.path());
        fs::create_dir_all(&plan.roots[0]).unwrap();
        let lock = plan.request().unwrap();
        drop(lock);
        fs::write(&plan.marker, b"invalid").unwrap();
        assert!(plan.finish_pending().is_err());
        assert!(plan.marker.exists());
        assert!(plan.roots[0].exists());
    }

    #[cfg(windows)]
    #[test]
    fn reset_keeps_pending_marker_when_windows_files_are_in_use_and_retries() {
        use std::os::windows::fs::OpenOptionsExt;
        let temp = tempfile::tempdir().unwrap();
        let plan = plan(temp.path());
        for root in &plan.roots {
            fs::create_dir_all(root).unwrap();
        }
        let blocked = plan.roots.last().unwrap().join("in-use.db");
        fs::write(&blocked, b"old").unwrap();
        let handle = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&blocked)
            .unwrap();
        drop(plan.request().unwrap());
        assert!(plan.finish_with_timeout(Duration::from_millis(60)).is_err());
        assert!(plan.marker.exists());
        assert!(blocked.exists());
        drop(handle);
        assert!(plan.finish_pending().unwrap());
        assert!(!plan.marker.exists());
        assert!(plan.roots.iter().all(|root| !root.exists()));
    }

    #[cfg(unix)]
    #[test]
    fn reset_unlinks_nested_symlinks_without_touching_their_targets() {
        let temp = tempfile::tempdir().unwrap();
        let plan = plan(temp.path());
        let outside = temp.path().join("other-application");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("keep"), b"other application").unwrap();
        for root in &plan.roots {
            fs::create_dir_all(root).unwrap();
        }
        std::os::unix::fs::symlink(&outside, plan.roots[0].join("linked-cache")).unwrap();
        drop(plan.request().unwrap());
        assert!(plan.finish_pending().unwrap());
        assert_eq!(
            fs::read(outside.join("keep")).unwrap(),
            b"other application"
        );
    }

    #[cfg(unix)]
    #[test]
    fn reset_rejects_redirected_roots_before_deleting_any_storage() {
        let temp = tempfile::tempdir().unwrap();
        let plan = plan(temp.path());
        let outside = temp.path().join("unrelated");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("keep"), b"keep").unwrap();
        for root in &plan.roots {
            fs::create_dir_all(root).unwrap();
        }
        drop(plan.request().unwrap());
        let root = plan.roots.last().unwrap();
        fs::remove_dir(root).unwrap();
        std::os::unix::fs::symlink(&outside, root).unwrap();
        assert!(plan.finish_pending().is_err());
        assert!(plan.roots[0].exists());
        assert!(outside.join("keep").exists());
        assert!(plan.marker.exists());
    }
}
