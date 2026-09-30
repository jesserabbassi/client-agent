use std::path::PathBuf;

fn beside_executable() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    executable
        .ancestors()
        .skip(1)
        .take(6)
        .map(|directory| directory.join(".env"))
        .find(|path| path.is_file())
}

pub(crate) fn load() {
    if let Some(path) = beside_executable() {
        if let Err(error) = dotenvy::from_path(&path) {
            tracing::warn!(%error, path = %path.display(), "could not read client configuration");
        }
    } else if let Err(error) = dotenvy::dotenv() {
        tracing::debug!(%error, "no client .env file found");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_quoted_app_path_with_spaces() {
        let path = std::env::temp_dir().join(format!("ninety-env-{}.txt", uuid::Uuid::new_v4()));
        std::fs::write(
            &path,
            "NINETY_GAME_LOL=\"/Applications/Example Game.app\"\n",
        )
        .expect("write test configuration");
        let configured_app = dotenvy::from_path_iter(&path)
            .expect("read .env")
            .map(|entry| entry.expect("valid .env entry"))
            .find(|(key, _)| key == "NINETY_GAME_LOL")
            .map(|(_, value)| value)
            .expect("game path configured");
        assert_eq!(configured_app, "/Applications/Example Game.app");
        std::fs::remove_file(path).expect("remove test configuration");
    }
}
