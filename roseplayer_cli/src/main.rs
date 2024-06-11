mod error;
mod player;

use crate::error::ApplicationError;
use crate::player::AudioPlayer;

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    // 初始化播放器并开始播放
    let audio_player = AudioPlayer::new("playlist.toml").await?;
    audio_player.play_playlist().await?;
    Ok(())
}
