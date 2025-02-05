mod bilibili;
mod dbus;
mod error;
mod player;
mod temp_dbus;

use crate::error::App;
use crate::player::playlist::PlayMode;
use crate::player::Audio;
use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};
use log::{error, warn};
use player::playlist::load;
use std::path::Path;
use std::process;
use std::sync::Arc;
use tikv_jemallocator::Jemalloc;
use tokio::fs;
use tokio::{
    sync::{mpsc, watch, Mutex},
    task,
};

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[tokio::main]
async fn main() -> Result<(), App> {
    let home_dir = std::env::var("HOME").map_err(|e| {
        App::Io(
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Failed to get HOME environment variable: {e}"),
            )
            .to_string(),
        )
    })?;

    // Define the required directories
    let required_dirs = [
        format!("{home_dir}/.config/rosesong/logs"),
        format!("{home_dir}/.config/rosesong/playlists"),
    ];

    // Ensure all directories exist
    for dir in &required_dirs {
        fs::create_dir_all(dir).await?;
    }

    // Check if playlist.toml exists, if not, create an empty one
    let playlist_path = format!("{home_dir}/.config/rosesong/playlists/playlist.toml");
    if !Path::new(&playlist_path).exists() {
        fs::write(&playlist_path, "").await?;
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

    // Check if the playlist is empty
    {
        let playlist_content = fs::read_to_string(&playlist_path).await?;
        if playlist_content.trim().is_empty() {
            warn!("Current playlist is empty");
            let (stop_sender, stop_receiver) = watch::channel(());
            let _ = start_temp_dbus_listener(stop_sender).await;
            wait_for_stop_signal(stop_receiver).await;
            let playlist_content = fs::read_to_string(&playlist_path).await?;
            if playlist_content.trim().is_empty() {
                process::exit(0);
            }
        }
    }

    load(&playlist_path).await?;
    let (stop_sender, stop_receiver) = watch::channel(());
    let _audio_player = start_player_and_dbus_listener(stop_sender).await?;
    wait_for_stop_signal(stop_receiver).await;
    process::exit(0);
}

async fn wait_for_stop_signal(mut stop_receiver: watch::Receiver<()>) {
    stop_receiver.changed().await.unwrap();
}

async fn start_temp_dbus_listener(
    stop_signal: watch::Sender<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    let stop_receiver = stop_signal.subscribe();

    task::spawn({
        let stop_signal = stop_signal.clone();
        async move {
            let result = temp_dbus::run_temp_dbus_server(stop_signal).await;
            if let Err(e) = result {
                error!("Temp DBus listener error: {}", e);
            }
        }
    });

    // Wait for the stop signal
    wait_for_stop_signal(stop_receiver).await;

    Ok(())
}

async fn start_player_and_dbus_listener(stop_signal: watch::Sender<()>) -> Result<Audio, App> {
    let play_mode = PlayMode::Loop;
    let initial_track_index = 0;
    let (command_sender, command_receiver) = mpsc::channel(1);

    let audio_player = Audio::new(
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
