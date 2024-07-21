mod bilibili;
mod dbus;
mod error;
mod player;

use crate::error::ApplicationError;
use crate::player::playlist::PlayMode;
use crate::player::AudioPlayer;
use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};
use std::fs;
use std::path::Path;
use std::process;
use std::sync::Arc;
use tokio::{
    sync::{mpsc, watch, Mutex},
    task,
};

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

    // Start the player and dbus listener
    let (stop_sender, stop_receiver) = watch::channel(());
    let _audio_player = start_player_and_dbus_listener(stop_sender).await?;

    // Wait for the stop signal
    wait_for_stop_signal(stop_receiver).await;

    process::exit(0);
}

async fn wait_for_stop_signal(mut stop_receiver: watch::Receiver<()>) {
    stop_receiver.changed().await.unwrap();
}

async fn start_player_and_dbus_listener(
    stop_signal: watch::Sender<()>,
) -> Result<AudioPlayer, ApplicationError> {
    let play_mode = PlayMode::Loop;
    let initial_track_index = 0;
    let (command_sender, command_receiver) = mpsc::channel(1);

    let audio_player = AudioPlayer::new(
        play_mode,
        initial_track_index,
        Arc::new(Mutex::new(command_receiver)),
    )
    .await?;

    task::spawn({
        let command_sender = command_sender.clone();
        let stop_signal = stop_signal.clone();
        async move {
            let _ = dbus::run_dbus_server(command_sender, stop_signal).await;
        }
    });

    task::spawn({
        let audio_player = audio_player.clone();
        async move {
            audio_player.play_playlist().await.unwrap();
        }
    });

    Ok(audio_player)
}
