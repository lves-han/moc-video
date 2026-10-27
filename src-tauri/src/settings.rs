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

pub fn defaults() -> Value {
    json!({"schema_version": 2, "last_source_key": null, "volume": 1.0, "muted": false,
        "auto_next": true, "window_bounds": null, "source_configs": [], "ui_locale": "en", "color_palette": "lime"})
}

pub fn validate(settings: &Value) -> Result<(), ApiError> {
    Contracts::get().validate("Settings", settings)?;
    let mut ids = HashSet::new();
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
                    let mut settings: Value = serde_json::from_slice(&data)
                        .map_err(|_| ApiError::internal("Stored settings JSON is damaged"))?;
                    let migrate = settings["schema_version"] == 1;
                    if migrate {
                        if settings.get("source_configs").is_some() {
                            return Err(ApiError::bad("Settings v1 must omit source_configs"));
                        }
                        settings["schema_version"] = json!(2);
                        settings["source_configs"] = json!([]);
                    } else if settings["schema_version"] != 2 {
                        return Err(ApiError::bad("Unsupported settings schema_version"));
                    }
                    validate(&settings)?;
                    if migrate {
                        store.save(&settings)?;
                    }
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
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok((store, defaults())),
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
        temporary
            .persist(&self.path)
            .map_err(|_| ApiError::internal("Settings atomic replacement failed"))?;
        Ok(())
    }
}
