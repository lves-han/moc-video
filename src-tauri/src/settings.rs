use crate::contracts::{ApiError, Contracts};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
use uuid::Uuid;

pub const SETTINGS_LIMIT: u64 = 1024 * 1024;

// The built-in TMDB source keeps one identity across language and instance
// changes, so history and the saved metadata choice stay attached to it.
pub const TMDB_CONFIG_ID: &str = "5f0c6a8e-2d4b-4e7a-9c1f-3b8d7e6a5c40";
const TMDB_INSTANCE: &str = "https://tmdb.elfhosted.com";

pub fn defaults() -> Value {
    json!({"schema_version": 2, "last_source_key": null, "volume": 1.0, "muted": false,
        "auto_next": true, "window_bounds": null, "source_configs": [], "ui_locale": "en", "color_palette": "lime",
        "tmdb": {"enabled": true, "instance_url": TMDB_INSTANCE, "language": "follow"}})
}

fn tmdb(settings: &Value) -> Value {
    settings
        .get("tmdb")
        .cloned()
        .unwrap_or_else(|| defaults()["tmdb"].clone())
}

/// Language used for Stremio metadata: the TMDB choice, or the app language when it follows.
pub fn metadata_language(settings: &Value) -> &'static str {
    let tmdb = tmdb(settings);
    let chosen = tmdb["language"].as_str().unwrap_or("follow");
    crate::host::stremio::locale(if chosen == "follow" {
        settings["ui_locale"].as_str().unwrap_or("en")
    } else {
        chosen
    })
}

/// Source configurations the host registers: user sources, then the built-in TMDB source.
pub fn source_configs(settings: &Value) -> Vec<Value> {
    let tmdb = tmdb(settings);
    let instance = tmdb["instance_url"]
        .as_str()
        .unwrap_or(TMDB_INSTANCE)
        .trim_end_matches('/');
    let mut configs = settings["source_configs"]
        .as_array()
        .expect("Validated configs")
        .clone();
    let service = &settings["playback_service"];
    let service_revision = if service["enabled"] == true {
        use sha2::{Digest, Sha256};
        Some(format!(
            "{:x}",
            Sha256::digest(format!(
                "{}\0{}",
                service["url"].as_str().unwrap_or(""),
                service["token"].as_str().unwrap_or("")
            ))
        ))
    } else {
        None
    };
    if service["enabled"] == true {
        configs.retain_mut(|config| {
            if config["kind"] != "stremio" {
                return true;
            }
            let group = service["groups"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|group| group["config_id"] == config["config_id"]);
            let Some(group) = group else {
                return false;
            };
            let Some(addons) = group["addons"].as_array() else {
                return false;
            };
            let Some(originals) = config["manifest_urls"].as_array() else {
                return false;
            };
            if group["status"] != "ready"
                || addons.len() != originals.len()
                || !addons
                    .iter()
                    .zip(originals)
                    .all(|(addon, url)| addon["original_url"] == *url)
            {
                return false;
            }
            let urls: Vec<String> = addons
                .iter()
                .filter_map(|addon| {
                    addon["source_id"].as_str().map(|id| {
                        format!("https://{}/{id}/en-US/manifest.json", crate::service::HOST)
                    })
                })
                .collect();
            if urls.len() != originals.len() {
                return false;
            }
            config["manifest_urls"] = json!(urls);
            config["service_addons"] = json!(addons);
            config["service_revision"] = json!(service_revision);
            true
        });
    }
    let mut config = json!({"kind": "stremio", "config_id": TMDB_CONFIG_ID, "name": "TMDB",
        "enabled": tmdb["enabled"] == true, "manifest_urls": [format!("{instance}/en-US/manifest.json")], "tmdb": true});
    // API mode answers through a virtual addon; the fingerprint re-syncs on key/endpoint changes.
    if service["enabled"] == true {
        config["manifest_urls"] = json!([format!(
            "https://{}/tmdb/en-US/manifest.json",
            crate::service::HOST
        )]);
        config["service_revision"] = json!(service_revision);
    } else if let Some(fingerprint) = crate::host::tmdb_api::fingerprint(settings) {
        config["manifest_urls"] = json!([crate::host::tmdb_api::manifest_url()]);
        config["api"] = json!(fingerprint);
    }
    configs.push(config);
    configs
}

fn initial_defaults() -> Value {
    let mut settings = defaults();
    settings["source_configs"] =
        serde_json::from_str(include_str!("../../src/data/defaultSources.json"))
            .expect("Bundled source defaults must be valid JSON");
    settings
}

pub fn redact_for_ui(settings: &Value) -> Value {
    let mut out = settings.clone();
    if let Some(service) = out
        .get_mut("playback_service")
        .and_then(Value::as_object_mut)
    {
        let configured = service
            .get("token")
            .and_then(Value::as_str)
            .is_some_and(|t| !t.trim().is_empty());
        service.insert("token".into(), json!(""));
        service.insert("token_configured".into(), json!(configured));
    }
    out
}

pub fn merge_preserved_token(next: &Value, current: &Value) -> Result<Value, ApiError> {
    let mut out = next.clone();
    if let Some(service) = out
        .get_mut("playback_service")
        .and_then(Value::as_object_mut)
    {
        service.remove("token_configured");
        let incoming_empty = service
            .get("token")
            .and_then(Value::as_str)
            .is_none_or(|t| t.trim().is_empty());
        if incoming_empty {
            if let Some(existing) = current
                .get("playback_service")
                .and_then(|s| s.get("token"))
                .and_then(Value::as_str)
                .filter(|t| !t.trim().is_empty())
            {
                let old_address = current["playback_service"]["url"].as_str().unwrap_or("");
                let new_address = service.get("url").and_then(Value::as_str).unwrap_or("");
                if !crate::service::same_service_address(old_address, new_address) {
                    return Err(ApiError::bad("Enter a token when changing the service address / 更换服务地址时请输入令牌"));
                }
                service.insert("token".into(), json!(existing));
            }
        }
    }
    Ok(out)
}

pub fn validate(settings: &Value) -> Result<(), ApiError> {
    Contracts::get().validate("Settings", settings)?;
    if let Some(service) = settings.get("playback_service") {
        crate::service::Config::parse(
            service["url"].as_str().expect("Validated service URL"),
            service["token"].as_str().expect("Validated token"),
        )?;
        let mut groups = HashSet::new();
        for group in service["groups"].as_array().expect("Validated groups") {
            if !groups.insert(group["config_id"].as_str().expect("Validated group ID")) {
                return Err(ApiError::bad("Duplicate service group"));
            }
        }
    }
    for field in ["instance_url", "api_url", "image_url"] {
        if let Some(url) = settings["tmdb"][field].as_str() {
            validate_url(url)?;
        }
    }
    let mut ids = HashSet::from([Uuid::parse_str(TMDB_CONFIG_ID).expect("Static UUID")]);
    for config in settings["source_configs"]
        .as_array()
        .expect("Validated configs")
    {
        let id = Uuid::parse_str(config["config_id"].as_str().expect("Validated UUID"))
            .map_err(|_| ApiError::bad("Invalid config_id"))?;
        if !ids.insert(id) {
            return Err(ApiError::bad("Duplicate config_id"));
        }
        match config["kind"].as_str() {
            Some("maccms") => validate_url(config["api_url"].as_str().expect("Validated URL"))?,
            Some("stremio") => {
                for url in config["manifest_urls"]
                    .as_array()
                    .expect("Validated manifests")
                {
                    validate_url(url.as_str().expect("Validated URL"))?;
                }
            }
            _ => return Err(ApiError::bad("Unsupported HTTP configuration kind")),
        }
    }
    for path in settings["local_paths"].as_array().into_iter().flatten() {
        if !Path::new(path.as_str().expect("Validated path")).is_absolute() {
            return Err(ApiError::bad("Local library paths must be absolute"));
        }
    }
    let mut live_ids = HashSet::new();
    for config in settings["live_configs"].as_array().into_iter().flatten() {
        let id = Uuid::parse_str(config["config_id"].as_str().expect("Validated UUID"))
            .map_err(|_| ApiError::bad("Invalid live config_id"))?;
        if !live_ids.insert(id) {
            return Err(ApiError::bad("Duplicate live config_id"));
        }
        validate_url(config["url"].as_str().expect("Validated URL"))?;
        if let Some(epg) = config["epg_url"].as_str() {
            validate_url(epg)?;
        }
    }
    if serde_json::to_vec(settings)
        .map_err(|_| ApiError::bad("Invalid settings JSON"))?
        .len()
        .saturating_add(1)
        > SETTINGS_LIMIT as usize
    {
        return Err(ApiError::new("RESOURCE_LIMIT", "Settings exceed one MiB"));
    }
    Ok(())
}

fn validate_url(raw: &str) -> Result<(), ApiError> {
    if raw.chars().any(|c| c.is_whitespace() || c.is_control())
        || raw.contains('\\')
        || !(raw.starts_with("http://") || raw.starts_with("https://"))
    {
        return Err(ApiError::bad(
            "HTTP configuration requires an absolute http/https URL",
        ));
    }
    let authority = raw
        .split_once("://")
        .expect("Checked scheme")
        .1
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    if authority.is_empty() || authority.contains('@') {
        return Err(ApiError::bad(
            "HTTP configuration requires a host without userinfo",
        ));
    }
    let url = url::Url::parse(raw).map_err(|_| ApiError::bad("Invalid HTTP configuration URL"))?;
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(ApiError::bad(
            "HTTP configuration URL must have a host and omit userinfo and fragment",
        ));
    }
    Ok(())
}

pub struct SettingsFile {
    pub path: PathBuf,
}
impl SettingsFile {
    pub fn load(path: PathBuf) -> Result<(Self, Value), ApiError> {
        let store = Self { path };
        match fs::File::open(&store.path) {
            Ok(file) => {
                let mut data = Vec::new();
                file.take(SETTINGS_LIMIT + 1)
                    .read_to_end(&mut data)
                    .map_err(|_| ApiError::internal("Settings read failed"))?;
                let result = (|| {
                    if data.len() > SETTINGS_LIMIT as usize {
                        return Err(ApiError::new(
                            "RESOURCE_LIMIT",
                            "Stored settings exceed one MiB",
                        ));
                    }
                    let settings: Value = serde_json::from_slice(&data)
                        .map_err(|_| ApiError::internal("Stored settings JSON is damaged"))?;
                    if settings["schema_version"] != 2 {
                        return Err(ApiError::bad("Unsupported settings schema_version"));
                    }
                    validate(&settings)?;
                    Ok(settings)
                })();
                match result {
                    Ok(settings) => Ok((store, settings)),
                    Err(error) => {
                        // Preserve the exact bytes before startup reports the fault.
                        let diagnostic = store
                            .path
                            .with_extension(format!("diagnostic-{}.json", Uuid::new_v4()));
                        fs::copy(&store.path, diagnostic).map_err(|_| {
                            ApiError::internal("Settings invalid; diagnostic copy failed")
                        })?;
                        Err(error)
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let settings = initial_defaults();
                validate(&settings)?;
                store.save(&settings)?;
                Ok((store, settings))
            }
            Err(_) => Err(ApiError::internal("Settings file open failed")),
        }
    }

    pub fn save(&self, settings: &Value) -> Result<(), ApiError> {
        self.save_checked(settings, || Ok(()))
    }

    pub fn save_checked(
        &self,
        settings: &Value,
        before_replace: impl FnOnce() -> Result<(), ApiError>,
    ) -> Result<(), ApiError> {
        validate(settings)?;
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|_| ApiError::internal("Settings directory creation failed"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
                .map_err(|_| ApiError::internal("Settings directory privacy setup failed"))?;
        }
        let mut temporary = NamedTempFile::new_in(parent)
            .map_err(|_| ApiError::internal("Settings temporary file creation failed"))?;
        serde_json::to_writer(&mut temporary, settings)
            .map_err(|_| ApiError::internal("Settings serialization failed"))?;
        temporary
            .write_all(b"\n")
            .map_err(|_| ApiError::internal("Settings temporary write failed"))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| ApiError::internal("Settings temporary sync failed"))?;
        before_replace()?;
        #[cfg(windows)]
        let result = persist_windows(temporary, &self.path);
        #[cfg(not(windows))]
        let result = temporary
            .persist(&self.path)
            .map(|_| ())
            .map_err(|error| error.error);
        result.map_err(|error| {
            let failure = ApiError::internal("Settings atomic replacement failed");
            #[cfg(windows)]
            let failure = failure
                .with_details(json!({"stage":"settings_replace", "os_code":error.raw_os_error()}));
            #[cfg(not(windows))]
            let _ = error;
            failure
        })?;
        Ok(())
    }
}

#[cfg(windows)]
fn persist_windows(temporary: NamedTempFile, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_NORMAL, SetFileAttributesW};
    let name: Vec<u16> = temporary
        .path()
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // Match tempfile's durable attributes, then use std's FileRenameInfoEx support
    // for replacing a file with open readers. Tempfile 3.27 uses only MoveFileExW.
    if unsafe { SetFileAttributesW(name.as_ptr(), FILE_ATTRIBUTE_NORMAL) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // RAII cleans the temporary path on failure; the destination is never removed first.
    fs::rename(temporary.path(), destination)
}

#[cfg(test)]
mod preset_tests {
    use super::*;

    #[test]
    fn preserved_service_token_is_bound_to_the_same_address_and_path() {
        let mut current = defaults();
        current["playback_service"] = json!({"enabled":true,"url":"http://127.0.0.1:18801/base","token":"saved-token","groups":[]});
        let mut next = redact_for_ui(&current);
        next["playback_service"]["url"] = json!("http://127.0.0.1:18801/base/");
        assert_eq!(merge_preserved_token(&next, &current).unwrap()["playback_service"]["token"], "saved-token");
        for address in ["http://127.0.0.1:18802/base", "http://127.0.0.1:18801/other", "https://127.0.0.1:18801/base"] {
            next["playback_service"]["url"] = json!(address);
            assert!(merge_preserved_token(&next, &current).is_err(), "{address}");
        }
        next["playback_service"]["token"] = json!("new-token");
        assert_eq!(merge_preserved_token(&next, &current).unwrap()["playback_service"]["token"], "new-token");
    }

    #[test]
    fn atomic_replacement_keeps_open_reader_snapshots() {
        let folder = tempfile::tempdir().unwrap();
        let store = SettingsFile {
            path: folder.path().join("settings.json"),
        };
        let mut settings = defaults();
        store.save(&settings).unwrap();
        for index in 0..256 {
            let mut previous = fs::File::open(&store.path).unwrap();
            let expected_previous = settings.clone();
            settings["volume"] = json!(if index % 2 == 0 { 0.25 } else { 0.75 });
            store
                .save(&settings)
                .unwrap_or_else(|error| panic!("replacement {index}: {error:?}"));
            let mut bytes = Vec::new();
            previous.read_to_end(&mut bytes).unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&bytes).unwrap(),
                expected_previous
            );
            assert_eq!(
                serde_json::from_slice::<Value>(&fs::read(&store.path).unwrap()).unwrap(),
                settings
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn atomic_replacement_reports_denied_access_without_changing_settings() {
        use std::os::windows::fs::OpenOptionsExt;
        let folder = tempfile::tempdir().unwrap();
        let store = SettingsFile {
            path: folder.path().join("settings.json"),
        };
        let mut settings = defaults();
        store.save(&settings).unwrap();
        let original = fs::read(&store.path).unwrap();
        settings["volume"] = json!(0.5);
        let blocked = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&store.path)
            .unwrap();
        let error = store.save(&settings).unwrap_err();
        assert_eq!(error.code, "INTERNAL_ERROR");
        assert_eq!(error.details.unwrap()["stage"], "settings_replace");
        assert_eq!(fs::read(&store.path).unwrap(), original);
        drop(blocked);
        let permissions = fs::metadata(&store.path).unwrap().permissions();
        let mut readonly = permissions.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&store.path, readonly).unwrap();
        let result = store.save(&settings);
        fs::set_permissions(&store.path, permissions).unwrap();
        assert!(result.is_err());
        assert_eq!(fs::read(&store.path).unwrap(), original);
        store.save(&settings).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&store.path).unwrap()).unwrap(),
            settings
        );
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 1);
    }

    #[test]
    fn font_preferences_accept_only_packaged_options_and_preserve_existing_profiles() {
        let mut settings = defaults();
        validate(&settings).unwrap();
        for family in ["noto-sans-sc", "noto-serif-sc"] {
            settings["font_family"] = json!(family);
            for scale in [90, 100, 110, 120] {
                settings["font_scale"] = json!(scale);
                validate(&settings).unwrap();
            }
        }
        settings["font_family"] = json!("system");
        assert!(validate(&settings).is_err());
        settings["font_family"] = json!("noto-sans-sc");
        settings["font_scale"] = json!(130);
        assert!(validate(&settings).is_err());
    }

    #[test]
    fn bundled_sources_are_valid_and_persist_without_duplication() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("settings.json");
        let (_, first) = SettingsFile::load(path.clone()).unwrap();
        validate(&first).unwrap();
        assert_eq!(first["source_configs"].as_array().unwrap().len(), 8);
        assert!(
            first["source_configs"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["kind"] == "maccms")
        );
        let configs = source_configs(&first);
        assert_eq!(configs[8]["config_id"], TMDB_CONFIG_ID);
        assert_eq!(
            configs[8]["manifest_urls"][0],
            "https://tmdb.elfhosted.com/en-US/manifest.json"
        );
        assert_eq!(SettingsFile::load(path).unwrap().1, first);
    }

    #[test]
    fn existing_empty_profile_is_not_migrated_or_reseeded() {
        let folder = tempfile::tempdir().unwrap();
        let store = SettingsFile {
            path: folder.path().join("settings.json"),
        };
        let mut current = defaults();
        current["volume"] = json!(0.25);
        store.save(&current).unwrap();
        assert_eq!(SettingsFile::load(store.path).unwrap().1, current);
    }

    #[test]
    fn tmdb_language_follows_app_unless_chosen() {
        let mut settings = defaults();
        assert_eq!(metadata_language(&settings), "en");
        settings["ui_locale"] = json!("zh-CN");
        assert_eq!(metadata_language(&settings), "zh-CN");
        settings["tmdb"]["language"] = json!("en");
        assert_eq!(metadata_language(&settings), "en");
        settings["tmdb"]["instance_url"] = json!("https://tmdb.example.test/base/");
        assert_eq!(
            source_configs(&settings)[0]["manifest_urls"][0],
            "https://tmdb.example.test/base/en-US/manifest.json"
        );
        let live = json!({"config_id": "6a1d2c3b-4e5f-4a6b-8c7d-9e0f1a2b3c4d", "name": "Live", "url": "https://live.test/list.m3u", "enabled": true});
        settings["live_configs"] = json!([live.clone()]);
        validate(&settings).unwrap();
        settings["live_configs"] = json!([live.clone(), live.clone()]);
        assert!(validate(&settings).is_err(), "duplicate live id");
        settings["live_configs"] = json!([{"config_id": "6a1d2c3b-4e5f-4a6b-8c7d-9e0f1a2b3c4d", "name": "Live", "url": "rtp://239.0.0.1:5000", "enabled": true}]);
        assert!(
            validate(&settings).is_err(),
            "live list URL must be http(s)"
        );
        settings["live_configs"] = json!([]);
        settings["source_configs"] = json!([{"kind": "maccms", "config_id": TMDB_CONFIG_ID, "name": "x", "enabled": true, "api_url": "https://a.test/api"}]);
        assert!(validate(&settings).is_err());
    }
}
