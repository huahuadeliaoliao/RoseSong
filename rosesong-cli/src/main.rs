mod bilibili;
mod cli;
mod error;
mod player;

use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use crate::error::ApplicationError;
use crate::player::playlist::PlayMode;
use crate::player::AudioPlayer;

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    let home_dir = match std::env::var("HOME") {
        Ok(dir) => dir,
        Err(e) => {
            return Err(ApplicationError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Failed to get HOME environment variable: {}", e),
            )))
        }
    };

    Logger::try_with_str("info")?
        .log_to_file(FileSpec::default().directory(format!("{}/.config/rosesong/logs", home_dir)))
        .rotate(
            Criterion::Size(1_000_000),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(3),
        )
        .duplicate_to_stderr(Duplicate::None)
        .start()?;

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    let (tx, rx) = mpsc::channel();

    let cli_thread = thread::spawn({
        let running_clone = running_clone.clone();
        let tx = tx.clone();
        move || {
            if let Err(e) = cli::run_cli(running_clone, tx, rx) {
                eprintln!("CLI error: {}", e);
            }
        }
    });

    let play_mode = PlayMode::Shuffle; // 默认循环播放模式
    let initial_track_index = 0; // 默认播放序号为 0

    let audio_player = AudioPlayer::new(play_mode, initial_track_index).await?;
    audio_player.play_playlist().await?;

    running.store(false, Ordering::Relaxed);
    if let Err(e) = cli_thread.join() {
        eprintln!("Error joining CLI thread: {:?}", e);
    }

    Ok(())
}
