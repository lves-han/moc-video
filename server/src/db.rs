use std::{fs::OpenOptions, io, path::Path};

pub fn initialize(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let path = dir.join("service.sqlite3");
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    drop(options.open(&path).map_err(|error| {
        io::Error::new(error.kind(), format!("opening service database: {error}"))
    })?);
    let conn = rusqlite::Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(())
}
