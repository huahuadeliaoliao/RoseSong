mod bilibili;
mod cli;
mod error;
mod player;

use flexi_logger::{Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crate::error::ApplicationError;
use crate::player::AudioPlayer;

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    // 使用 flexi_logger 初始化日志配置
    Logger::try_with_str("info")?
        .log_to_file(
            FileSpec::default().directory(std::env::var("HOME").unwrap() + "/.cache/rosesong"),
        )
        .rotate(
            Criterion::Size(1_000_000),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(3),
        )
        .duplicate_to_stderr(Duplicate::None)
        .start()?;

    // 创建一个 Arc<AtomicBool> 用于控制 CLI 线程的运行
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // 启动 CLI 线程
    let cli_thread = thread::spawn(move || {
        if let Err(e) = cli::run_cli(running_clone) {
            eprintln!("CLI error: {}", e);
        }
    });

    // 初始化播放器并开始播放
    let audio_player = AudioPlayer::new("playlist.toml").await?;
    audio_player.play_playlist().await?;

    // 等待 CLI 线程结束
    running.store(false, Ordering::Relaxed);
    if let Err(e) = cli_thread.join() {
        eprintln!("Error joining CLI thread: {:?}", e);
    }

    Ok(())
}
