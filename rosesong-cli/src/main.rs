mod bilibili;
mod error;
mod player;

use crate::error::ApplicationError;
use crate::player::playlist::PlayMode;
use crate::player::{AudioPlayer, PlayerCommand};
use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use zbus::ConnectionBuilder;
use zbus::{fdo, interface};

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

    // Create an unbuffered channel
    let (tx, rx) = mpsc::channel(64);
    let rx = Arc::new(Mutex::new(rx));
    let audio_player = Arc::new(RwLock::new(
        AudioPlayer::new(PlayMode::Loop, 0, rx.clone()).await?,
    ));

    // Start the D-Bus service
    let _connection = ConnectionBuilder::session()?
        .name("com.rosesong.Player")?
        .serve_at("/com/rosesong/Player", PlayerDbusService { tx: tx.clone() })?
        .build()
        .await?;

    tokio::try_join!(
        async {
            audio_player.write().await.play_playlist().await?;
            Ok::<(), ApplicationError>(())
        },
        async {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            #[allow(unreachable_code)]
            Ok::<(), ApplicationError>(())
        }
    )?;

    Ok(())
}

struct PlayerDbusService {
    tx: mpsc::Sender<PlayerCommand>,
}

#[interface(name = "com.rosesong.Player")]
impl PlayerDbusService {
    async fn play(&self) -> fdo::Result<()> {
        self.tx
            .send(PlayerCommand::Play)
            .await
            .map_err(|e| fdo::Error::Failed(format!("{}", e)))?;
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.tx
            .send(PlayerCommand::Pause)
            .await
            .map_err(|e| fdo::Error::Failed(format!("{}", e)))?;
        Ok(())
    }

    async fn next(&self) -> fdo::Result<()> {
        self.tx
            .send(PlayerCommand::Next)
            .await
            .map_err(|e| fdo::Error::Failed(format!("{}", e)))?;
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        self.tx
            .send(PlayerCommand::Previous)
            .await
            .map_err(|e| fdo::Error::Failed(format!("{}", e)))?;
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.tx
            .send(PlayerCommand::Stop)
            .await
            .map_err(|e| fdo::Error::Failed(format!("{}", e)))?;
        Ok(())
    }
}
