mod bilibili;
mod cli;
mod error;
mod player;

use crate::error::ApplicationError;
use crate::player::playlist::{load_first_track_index, PlayMode};
use crate::player::AudioPlayer;
use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    let home_dir = std::env::var("HOME").map_err(|e| {
        ApplicationError::IoError(
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Failed to get HOME environment variable: {}", e),
            )
            .to_string(),
        )
    })?;

    // Define the required directories
    let required_dirs = [
        format!("{}/.config/rosesong/logs", home_dir),
        format!("{}/.config/rosesong/favorites", home_dir),
        format!("{}/.config/rosesong/playlists", home_dir),
        format!("{}/.config/rosesong/settings", home_dir),
    ];

    // Ensure all directories exist
    for dir in &required_dirs {
        fs::create_dir_all(dir)?;
    }

    // Check if playlist.toml exists, if not, create an empty one
    let playlist_path = format!("{}/.config/rosesong/playlists/playlist.toml", home_dir);
    if !Path::new(&playlist_path).exists() {
        fs::write(&playlist_path, "")?;
    }

    // Logger setup
    Logger::try_with_str("info")?
        .log_to_file(FileSpec::default().directory(&required_dirs[0]))
        .rotate(
            Criterion::Size(1_000_000),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(3),
        )
        .duplicate_to_stderr(Duplicate::None)
        .start()?;

    let (tx, rx) = mpsc::channel();

    let cli_thread = thread::spawn(move || {
        if let Err(e) = cli::run_cli(tx, rx) {
            eprintln!("CLI error: {}", e);
        }
    });

    let play_mode = PlayMode::Loop; // 默认循环播放模式
    let initial_track_index = load_first_track_index(&playlist_path).await?;

    let audio_player = AudioPlayer::new(play_mode, initial_track_index).await?;
    audio_player.play_playlist().await?;

    if let Err(e) = cli_thread.join() {
        eprintln!("Error joining CLI thread: {:?}", e);
    }

    Ok(())
}
