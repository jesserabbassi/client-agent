use std::{
    path::{Path, PathBuf},
    process::Command,
};

struct GameTarget {
    env_key: &'static str,
    steam_id: Option<u32>,
}

fn target_for(name: &str) -> Option<GameTarget> {
    Some(match name {
        "Counter-Strike 2" => GameTarget {
            env_key: "NINETY_GAME_CS2",
            steam_id: Some(730),
        },
        "Valorant" => GameTarget {
            env_key: "NINETY_GAME_VALORANT",
            steam_id: None,
        },
        "League of Legends" => GameTarget {
            env_key: "NINETY_GAME_LOL",
            steam_id: None,
        },
        "Dota 2" => GameTarget {
            env_key: "NINETY_GAME_DOTA2",
            steam_id: Some(570),
        },
        "EA FC 24" => GameTarget {
            env_key: "NINETY_GAME_FC24",
            steam_id: None,
        },
        "Rocket League" => GameTarget {
            env_key: "NINETY_GAME_ROCKET_LEAGUE",
            steam_id: None,
        },
        _ => return None,
    })
}

pub(crate) fn launch_game(name: &str) -> Result<(), String> {
    let target = target_for(name).ok_or("Unknown game.")?;
    crate::infrastructure::local_env::load();
    let arguments = std::env::var(format!("{}_ARGS", target.env_key))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            serde_json::from_str::<Vec<String>>(&value).map_err(|_| {
                format!(
                    "Invalid {}_ARGS; use a JSON array of strings.",
                    target.env_key
                )
            })
        })
        .transpose()?;
    match std::env::var(target.env_key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        Some(value) if value.starts_with("steam://rungameid/") => {
            let id = value
                .trim_start_matches("steam://rungameid/")
                .parse::<u32>()
                .map_err(|_| format!("Invalid Steam game ID in {}.", target.env_key))?;
            launch_steam(id)
        }
        Some(value) => launch_path(Path::new(&value), arguments.as_deref().unwrap_or(&[])),
        None => match target.steam_id {
            Some(id) => launch_steam(id),
            None => Err(format!(
                "Game launcher not configured. Set {} to the installed game path.",
                target.env_key
            )),
        },
    }
}

fn launch_path(path: &Path, arguments: &[String]) -> Result<(), String> {
    if !path.is_absolute() || !path.exists() {
        return Err(format!("Game path does not exist: {}", path.display()));
    }
    #[cfg(target_os = "macos")]
    if path.extension().is_some_and(|ext| ext == "app") && path.is_dir() {
        return run_open(path, arguments);
    }
    #[cfg(windows)]
    if path
        .extension()
        .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("lnk"))
    {
        if !arguments.is_empty() {
            return Err(
                "Shortcut arguments are not supported; configure the executable instead.".into(),
            );
        }
        return Command::new("explorer.exe")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not open game shortcut: {error}"));
    }
    if !path.is_file() {
        return Err(format!(
            "Game path is not an executable file: {}",
            path.display()
        ));
    }
    Command::new(path)
        .args(arguments)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Could not start game: {error}"))
}

#[cfg(target_os = "macos")]
fn run_open(path: &Path, arguments: &[String]) -> Result<(), String> {
    let mut command = Command::new("open");
    command.arg("-a").arg(path);
    if !arguments.is_empty() {
        command.arg("--args").args(arguments);
    }
    let status = command
        .status()
        .map_err(|error| format!("Could not open game: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "The game launcher could not open the app.".into())
}

fn launch_steam(id: u32) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let candidates = [
            PathBuf::from("/Applications/Steam.app"),
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join("Applications/Steam.app"),
        ];
        let steam = candidates
            .iter()
            .find(|path| path.is_dir())
            .ok_or("Steam is not installed. Install it or configure the game's path.")?;
        let status = Command::new("open")
            .arg("-a")
            .arg(steam)
            .arg(format!("steam://rungameid/{id}"))
            .status()
            .map_err(|error| format!("Could not open Steam: {error}"))?;
        return status
            .success()
            .then_some(())
            .ok_or_else(|| "Steam could not launch the game.".into());
    }
    #[cfg(windows)]
    {
        let steam = ["ProgramFiles(x86)", "ProgramFiles"]
            .iter()
            .filter_map(|key| std::env::var_os(key))
            .map(|folder| PathBuf::from(folder).join("Steam/steam.exe"))
            .find(|path| path.is_file())
            .ok_or("Steam is not installed. Install it or configure the game's path.")?;
        return Command::new(steam)
            .arg("-applaunch")
            .arg(id.to_string())
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not start Steam game: {error}"));
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        Command::new("steam")
            .arg("-applaunch")
            .arg(id.to_string())
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not start Steam game: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_targets_are_explicit() {
        assert_eq!(target_for("Counter-Strike 2").unwrap().steam_id, Some(730));
        assert_eq!(
            target_for("Valorant").unwrap().env_key,
            "NINETY_GAME_VALORANT"
        );
        assert!(target_for("Unknown game").is_none());
    }

    #[test]
    fn missing_executable_does_not_launch() {
        assert!(launch_path(Path::new("/missing/ninety-game"), &[]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn starts_configured_executable() {
        assert!(launch_path(Path::new("/usr/bin/true"), &[]).is_ok());
    }
}
