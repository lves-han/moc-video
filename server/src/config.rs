use serde::Deserialize;
use std::{
    env, fs,
    io::{self, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Lookup {
    pub handle_ttl_seconds: i64,
    pub line_cache_seconds: i64,
}
impl Default for Lookup {
    fn default() -> Self {
        Self {
            handle_ttl_seconds: 900,
            line_cache_seconds: 60,
        }
    }
}

#[derive(Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Service {
    listen: String,
    data_dir: PathBuf,
    token: Option<String>,
}
impl Default for Service {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:8787".into(),
            data_dir: "./data".into(),
            token: None,
        }
    }
}
#[derive(Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Tmdb {
    api_url: String,
    key: String,
}
impl Default for Tmdb {
    fn default() -> Self {
        Self {
            api_url: "https://api.themoviedb.org/3".into(),
            key: String::new(),
        }
    }
}
#[derive(Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Home {
    timezone: String,
    poll_seconds: u64,
}
impl Default for Home {
    fn default() -> Self {
        Self {
            timezone: "Asia/Shanghai".into(),
            poll_seconds: 60,
        }
    }
}
#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    service: Service,
    tmdb: Tmdb,
    home: Home,
    lookup: Lookup,
}

fn env_value(name: &str) -> Result<Option<String>, String> {
    match env::var(name) {
        Ok(v) => Ok(Some(v)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(_) => Err(format!("{name} is not valid UTF-8")),
    }
}
impl Settings {
    fn parse(bytes: Option<&[u8]>) -> Result<Self, String> {
        let mut s: Self = match bytes {
            Some(bytes) => toml::from_str(
                std::str::from_utf8(bytes).map_err(|_| "configuration must be UTF-8")?,
            )
            .map_err(|_| "invalid TOML configuration")?,
            None => Self::default(),
        };
        if let Some(v) = env_value("MOC_SERVICE_LISTEN")? {
            s.service.listen = v;
        }
        if let Some(v) = env::var_os("MOC_SERVICE_DATA_DIR") {
            s.service.data_dir = v.into();
        }
        if let Some(v) = env_value("MOC_SERVICE_TOKEN")? {
            s.service.token = Some(v);
        }
        if let Some(v) = env_value("MOC_TMDB_API_URL")? {
            s.tmdb.api_url = v;
        }
        if let Some(v) = env_value("MOC_TMDB_KEY")? {
            s.tmdb.key = v;
        }
        if let Some(v) = env_value("TZ")? {
            s.home.timezone = v;
        }
        if let Some(v) = env_value("MOC_HOME_POLL_SECONDS")? {
            s.home.poll_seconds = v.parse().map_err(|_| "invalid MOC_HOME_POLL_SECONDS")?;
        }
        if let Some(v) = env_value("MOC_HANDLE_TTL_SECONDS")? {
            s.lookup.handle_ttl_seconds =
                v.parse().map_err(|_| "invalid MOC_HANDLE_TTL_SECONDS")?;
        }
        if let Some(v) = env_value("MOC_LINE_CACHE_SECONDS")? {
            s.lookup.line_cache_seconds =
                v.parse().map_err(|_| "invalid MOC_LINE_CACHE_SECONDS")?;
        }
        s.service
            .listen
            .parse::<SocketAddr>()
            .map_err(|_| "invalid service.listen")?;
        if s.service.data_dir.as_os_str().is_empty() {
            return Err("service.data_dir must not be empty".into());
        }
        if s.service
            .token
            .as_ref()
            .is_some_and(|v| v.trim().is_empty() || v.trim() != v)
        {
            return Err(
                "service.token must be non-empty and have no surrounding whitespace".into(),
            );
        }
        s.home
            .timezone
            .parse::<chrono_tz::Tz>()
            .map_err(|_| "invalid home.timezone")?;
        if s.home.poll_seconds == 0 {
            return Err("home.poll_seconds must be positive".into());
        }
        if !(1..=86_400).contains(&s.lookup.handle_ttl_seconds)
            || !(1..=86_400).contains(&s.lookup.line_cache_seconds)
        {
            return Err("lookup lifetimes must be between 1 and 86400 seconds".into());
        }
        crate::metadata::tmdb::validate_base(&s.tmdb.api_url).map_err(str::to_owned)?;
        Ok(s)
    }
}

fn read_config(path: &Path, required: bool) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound && !required => Ok(None),
        Err(e) => Err(e),
    }
}

pub struct Config {
    pub listen: SocketAddr,
    pub data_dir: PathBuf,
    pub token: String,
    pub tmdb_url: String,
    pub tmdb_key: String,
    pub timezone: chrono_tz::Tz,
    pub home_poll_seconds: u64,
    pub lookup: Arc<RwLock<Lookup>>,
    path: PathBuf,
    required: bool,
    initial: Settings,
    contents: Option<Vec<u8>>,
}
impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let explicit = env::var_os("MOC_SERVICE_CONFIG");
        let required = explicit.is_some();
        let path = PathBuf::from(explicit.unwrap_or_else(|| "service.toml".into()));
        let contents = read_config(&path, required)
            .map_err(|e| format!("cannot read configuration {}: {e}", path.display()))?;
        let settings = Settings::parse(contents.as_deref())
            .map_err(|e| format!("configuration {}: {e}", path.display()))?;
        fs::create_dir_all(&settings.service.data_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                &settings.service.data_dir,
                fs::Permissions::from_mode(0o700),
            )?;
        }
        let token = match &settings.service.token {
            Some(v) => v.clone(),
            None => load_or_create_token(&settings.service.data_dir)?,
        };
        Ok(Self {
            listen: settings.service.listen.parse()?,
            data_dir: settings.service.data_dir.clone(),
            token,
            tmdb_url: settings.tmdb.api_url.clone(),
            tmdb_key: settings.tmdb.key.clone(),
            timezone: settings.home.timezone.parse()?,
            home_poll_seconds: settings.home.poll_seconds,
            lookup: Arc::new(RwLock::new(settings.lookup.clone())),
            path,
            required,
            initial: settings,
            contents,
        })
    }
    pub fn watch(&self) {
        watch_file(
            self.path.clone(),
            self.required,
            self.initial.clone(),
            self.contents.clone(),
            self.lookup.clone(),
            std::time::Duration::from_secs(5),
        );
    }
}

fn watch_file(
    path: PathBuf,
    required: bool,
    initial: Settings,
    mut previous: Option<Vec<u8>>,
    lookup: Arc<RwLock<Lookup>>,
    interval: std::time::Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut read_error = false;
        loop {
            tokio::time::sleep(interval).await;
            let contents = match read_config(&path, required) {
                Ok(Some(v)) => {
                    read_error = false;
                    Some(v)
                }
                Ok(None) if previous.is_none() => continue,
                Ok(None) | Err(_) => {
                    if !read_error {
                        eprintln!(
                            "Cannot read configuration {}; keeping active settings",
                            path.display()
                        );
                        read_error = true;
                    }
                    continue;
                }
            };
            if contents == previous {
                continue;
            }
            previous = contents.clone();
            match Settings::parse(contents.as_deref()) {
                Ok(settings) => {
                    if settings.service != initial.service
                        || settings.tmdb != initial.tmdb
                        || settings.home != initial.home
                    {
                        eprintln!(
                            "Configuration updated; service, TMDB and home settings require restart"
                        );
                    }
                    *lookup.write().expect("lookup configuration") = settings.lookup;
                    eprintln!("Lookup configuration reloaded");
                }
                Err(e) => eprintln!(
                    "Invalid configuration {}: {e}; keeping active settings",
                    path.display()
                ),
            }
        }
    })
}

fn load_or_create_token(dir: &Path) -> io::Result<String> {
    let path = dir.join("service-token");
    if path.exists() {
        return read_token(&path);
    }
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    writeln!(temp, "{token}")?;
    temp.as_file().sync_all()?;
    match temp.persist_noclobber(&path) {
        Ok(_) => {
            eprintln!("Service token saved to {}", path.display());
            Ok(token)
        }
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => read_token(&path),
        Err(error) => Err(error.error),
    }
}

fn read_token(path: &Path) -> io::Result<String> {
    let token = fs::read_to_string(path)?;
    let token = token.trim();
    if token.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "service token file is empty",
        ));
    }
    Ok(token.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_accepts_sections_and_rejects_invalid_values() {
        let config = Settings::parse(Some(
            br#"
            [service]
            listen = "127.0.0.1:9876"
            [tmdb]
            api_url = "https://example.com/3"
            [home]
            timezone = "UTC"
            poll_seconds = 10
            [lookup]
            handle_ttl_seconds = 17
            line_cache_seconds = 23
        "#,
        ))
        .unwrap();
        assert_eq!(config.lookup.handle_ttl_seconds, 17);
        assert_eq!(config.lookup.line_cache_seconds, 23);
        assert!(Settings::parse(Some(b"[lookup]\nline_cache_seconds = 0")).is_err());
        assert!(Settings::parse(Some(b"[lookup]\nline_cache_second = 5")).is_err());
        assert!(Settings::parse(Some(b"[home]\ntimezone = 'invalid'")).is_err());
    }

    #[tokio::test]
    async fn changed_file_reloads_and_invalid_or_missing_file_preserves_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("service.toml");
        let initial = b"[lookup]\nhandle_ttl_seconds = 17\nline_cache_seconds = 23\n";
        fs::write(&path, initial).unwrap();
        let settings = Settings::parse(Some(initial)).unwrap();
        let lookup = Arc::new(RwLock::new(settings.lookup.clone()));
        let task = watch_file(
            path.clone(),
            true,
            settings,
            Some(initial.to_vec()),
            lookup.clone(),
            std::time::Duration::from_millis(20),
        );
        fs::write(
            &path,
            b"[lookup]\nhandle_ttl_seconds = 29\nline_cache_seconds = 31\n",
        )
        .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if lookup.read().unwrap().line_cache_seconds == 31 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        fs::write(&path, b"[lookup]\nline_cache_seconds = 0\n").unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        assert_eq!(lookup.read().unwrap().line_cache_seconds, 31);
        fs::remove_file(&path).unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        assert_eq!(lookup.read().unwrap().handle_ttl_seconds, 29);
        fs::write(
            &path,
            b"[lookup]\nhandle_ttl_seconds = 37\nline_cache_seconds = 41\n",
        )
        .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if lookup.read().unwrap().line_cache_seconds == 41 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        task.abort();
    }
}
