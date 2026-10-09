use rusqlite::{Connection, params};
use std::{path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

pub(crate) struct LocalDatabase {
    path: PathBuf,
    key: String,
}

impl LocalDatabase {
    pub(crate) fn open() -> Result<Self, &'static str> {
        let key = match super::secure_store::get("database-key")? {
            Some(key) if !key.is_empty() => key,
            Some(_) => return Err("database key is empty"),
            None => {
                let key = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, rand::random::<[u8; 32]>());
                super::secure_store::set("database-key", &key)?;
                key
            }
        };
        let path = data_path()?.join("ninety-agent.sqlcipher");
        let db = Self { path, key };
        db.with_connection(|connection| {
            connection.execute_batch(
                "CREATE TABLE IF NOT EXISTS telemetry_queue (
                    event_id TEXT PRIMARY KEY NOT NULL,
                    payload TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );",
            ).map_err(|_| "could not initialize encrypted local database")
        })?;
        Ok(db)
    }

    pub(crate) fn enqueue(&self, event_id: &str, payload: &str) -> Result<(), &'static str> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT OR IGNORE INTO telemetry_queue(event_id, payload, created_at) VALUES (?1, ?2, ?3)",
                params![event_id, payload, now_seconds()],
            ).map_err(|_| "could not queue telemetry")?;
            Ok(())
        })
    }

    pub(crate) fn remove(&self, event_id: &str) -> Result<(), &'static str> {
        self.with_connection(|connection| {
            connection.execute("DELETE FROM telemetry_queue WHERE event_id = ?1", params![event_id])
                .map_err(|_| "could not acknowledge queued telemetry")?;
            Ok(())
        })
    }

    pub(crate) fn pending(&self) -> Result<Vec<(String, String)>, &'static str> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT event_id, payload FROM telemetry_queue ORDER BY created_at LIMIT 100",
            ).map_err(|_| "could not read queued telemetry")?;
            let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(|_| "could not read queued telemetry")?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|_| "could not read queued telemetry")
        })
    }

    fn with_connection<T>(&self, operation: impl FnOnce(&Connection) -> Result<T, &'static str>) -> Result<T, &'static str> {
        let connection = Connection::open(&self.path).map_err(|_| "could not open encrypted local database")?;
        connection.pragma_update(None, "key", &self.key).map_err(|_| "could not unlock encrypted local database")?;
        operation(&connection)
    }
}

fn data_path() -> Result<PathBuf, &'static str> {
    if let Some(value) = std::env::var_os("NINETY_DATA_DIR") {
        let path = PathBuf::from(value);
        std::fs::create_dir_all(&path).map_err(|_| "could not create secure data directory")?;
        return Ok(path);
    }
    let path = std::env::current_exe().ok()
        .and_then(|executable| executable.parent().map(PathBuf::from))
        .ok_or("could not locate secure data directory")?;
    Ok(path)
}

fn now_seconds() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlcipher_database_is_not_readable_as_plain_sqlite() {
        let path = std::env::temp_dir().join(format!("ninety-sqlcipher-{}.db", uuid::Uuid::new_v4()));
        let key = "test-only-key";
        {
            let connection = Connection::open(&path).expect("create database");
            connection.pragma_update(None, "key", key).expect("set SQLCipher key");
            connection.execute("CREATE TABLE secret(value TEXT NOT NULL)", []).expect("create encrypted table");
            connection.execute("INSERT INTO secret(value) VALUES ('hidden')", []).expect("insert encrypted value");
        }
        let plain = Connection::open(&path).expect("open file as sqlite");
        assert!(plain.prepare("SELECT value FROM secret").is_err());
        let _ = std::fs::remove_file(path);
    }
}
