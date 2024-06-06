use glib::MainLoop;
use gstreamer_player::{Player, PlayerGMainContextSignalDispatcher, PlayerVideoRenderer};

// 导入音频URL模块
mod audio_url;
mod error;

use error::ApplicationError;

#[tokio::main]
async fn main() -> Result<(), ApplicationError> {
    // 初始化GStreamer
    gstreamer::init().map_err(|e| ApplicationError::InitError(e.to_string()))?;

    // 创建一个GStreamer Player
    let player = Player::new(
        None::<PlayerVideoRenderer>,
        Some(PlayerGMainContextSignalDispatcher::new(None)),
    );

    let bvid = "BV1Em411C7Sk";
    let cid = "1528594353";

    // 创建HTTP客户端
    let client = reqwest::Client::new();

    // 获取音频URL
    let audio_url = audio_url::fetch_audio_url(&client, bvid, cid).await?;

    // 设置媒体文件的URI
    player.set_uri(Some(&audio_url));

    // 连接播放器的状态更改信号以处理播放结束
    player.connect_end_of_stream(move |_| {
        println!("播放结束");
        // 在播放结束时退出
        std::process::exit(0);
    });

    // 开始播放
    player.play();

    // 保持主线程运行以便播放音频
    let main_loop = MainLoop::new(None, false);
    main_loop.run();

    Ok(())
}
