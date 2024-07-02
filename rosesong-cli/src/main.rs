mod bilibili;
mod cli;
mod error;
mod player;

use crate::error::ApplicationError;
use crate::player::playlist::PlayMode;
use crate::player::AudioPlayer;
use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};
use std::fs;
use std::sync::mpsc;
use std::thread;

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    let home_dir = std::env::var("HOME").map_err(|e| {
        ApplicationError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Failed to get HOME environment variable: {}", e),
        ))
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
        fs::create_dir_all(dir).map_err(|e| ApplicationError::IoError(e))?;
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

    let play_mode = PlayMode::Shuffle; // 默认循环播放模式
    let initial_track_index = 1; // 默认播放序号为 0

    let audio_player = AudioPlayer::new(play_mode, initial_track_index).await?;
    audio_player.play_playlist().await?;

    if let Err(e) = cli_thread.join() {
        eprintln!("Error joining CLI thread: {:?}", e);
    }

    Ok(())
}
