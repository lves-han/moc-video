use std::{
    env, fs,
    io::{self, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
};

pub struct Config {
    pub listen: SocketAddr,
    pub data_dir: PathBuf,
    pub token: String,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let listen = env::var("MOC_SERVICE_LISTEN")
            .unwrap_or_else(|_| "127.0.0.1:8787".into())
            .parse()?;
        let data_dir =
            PathBuf::from(env::var_os("MOC_SERVICE_DATA_DIR").unwrap_or_else(|| "./data".into()));
        fs::create_dir_all(&data_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&data_dir, fs::Permissions::from_mode(0o700))?;
        }
        let token = match env::var("MOC_SERVICE_TOKEN") {
            Ok(value) if !value.trim().is_empty() && value.trim() == value => value,
            Ok(_) => {
                return Err(
                    "MOC_SERVICE_TOKEN must be non-empty and have no surrounding whitespace".into(),
                );
            }
            Err(env::VarError::NotPresent) => load_or_create_token(&data_dir)?,
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            listen,
            data_dir,
            token,
        })
    }
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
