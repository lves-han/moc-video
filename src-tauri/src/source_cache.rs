//! Private bootstrap snapshots let the next launch reconstruct source catalogs
//! without contacting every provider before the first page can render.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

const LIMIT: u64 = 24 * 1024 * 1024;

pub struct SourceCache {
    directory: PathBuf,
}

impl SourceCache {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    fn path(&self, config: &Value) -> PathBuf {
        let encoded = serde_json::to_vec(config).expect("source configuration is JSON");
        self.directory
            .join(format!("{:x}.json", Sha256::digest(encoded)))
    }

    pub fn load(&self, config: &Value) -> Option<Value> {
        let file = fs::File::open(self.path(config)).ok()?;
        let mut bytes = Vec::new();
        if file.take(LIMIT + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > LIMIT {
            return None;
        }
        let value: Value = serde_json::from_slice(&bytes).ok()?;
        (value["version"] == 1 && value["kind"] == config["kind"]).then(|| value["payload"].clone())
    }

    pub fn save(&self, config: &Value, payload: Value) -> std::io::Result<()> {
        let bytes =
            serde_json::to_vec(&json!({"version": 1, "kind": config["kind"], "payload": payload}))?;
        if bytes.len() as u64 > LIMIT {
            return Err(std::io::Error::other(
                "source bootstrap cache exceeds size limit",
            ));
        }
        fs::create_dir_all(&self.directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.directory, fs::Permissions::from_mode(0o700))?;
        }
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist(self.path(config))
            .map_err(|error| error.error)?;
        Ok(())
    }
}
