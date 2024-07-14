mod bilibili;
mod error;
mod player;

use crate::error::ApplicationError;
use crate::player::playlist::PlayMode;
use crate::player::AudioPlayer;
use daemonize::Daemonize;
use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};
use std::fs;
use std::fs::File;
use std::path::Path;
use std::process::Command;

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

    let pid_file = "/tmp/rosesong.pid";

    // Check if the PID file exists and if the process is running
    if Path::new(pid_file).exists() {
        if let Ok(pid) = fs::read_to_string(pid_file) {
            if let Ok(_) = Command::new("kill").arg("-0").arg(pid.trim()).output() {
                println!("rosesong已经在后台运行");
                return Ok(());
            }
        }
    }

    // Daemonize setup
    let stdout = File::create("/tmp/daemon.out").unwrap();
    let stderr = File::create("/tmp/daemon.err").unwrap();

    let daemonize = Daemonize::new()
        .pid_file(pid_file) // Specify the location for the PID file
        .stdout(stdout) // Redirect stdout to a file
        .stderr(stderr) // Redirect stderr to a file
        .chown_pid_file(true); // Change the owner of the PID file to the current user

    match daemonize.start() {
        Ok(_) => println!("Daemon started successfully"),
        Err(e) => {
            eprintln!("Error, {}", e);
        }
    }

    let play_mode = PlayMode::Loop; // 默认循环播放模式
    let initial_track_index = 0;

    let audio_player = AudioPlayer::new(play_mode, initial_track_index).await?;
    audio_player.play_playlist().await?;

    Ok(())
}
