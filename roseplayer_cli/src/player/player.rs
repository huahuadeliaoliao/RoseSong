use crate::error::ApplicationError;
use crate::player::audio_url::fetch_audio_url;
use crate::player::playlist::{Playlist, Track};
use glib::MainLoop;
use gstreamer_player::{Player, PlayerGMainContextSignalDispatcher, PlayerVideoRenderer};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AudioPlayer {
    player: Player,
    client: Client,
    playlist: Arc<Mutex<Vec<Track>>>,
    current_track: Arc<Mutex<usize>>,
}

impl AudioPlayer {
    pub async fn new(playlist_path: &str) -> Result<Self, ApplicationError> {
        // 初始化GStreamer
        gstreamer::init().map_err(|e| ApplicationError::InitError(e.to_string()))?;

        // 创建一个GStreamer Player
        let player = Player::new(
            None::<PlayerVideoRenderer>,
            Some(PlayerGMainContextSignalDispatcher::new(None)),
        );

        let client = Client::new();

        // 加载播放列表
        let playlist = Playlist::load_from_file(playlist_path)?;
        let playlist = Arc::new(Mutex::new(playlist.tracks));
        let current_track = Arc::new(Mutex::new(0));

        Ok(Self {
            player,
            client,
            playlist,
            current_track,
        })
    }

    pub async fn play_playlist(&self) -> Result<(), ApplicationError> {
        {
            let playlist = Arc::clone(&self.playlist);
            let current_track = Arc::clone(&self.current_track);
            let player = self.player.clone();
            let client = self.client.clone();

            tokio::spawn(async move {
                loop {
                    let index = {
                        let mut current_track = current_track.lock().await;
                        if *current_track >= playlist.lock().await.len() {
                            break;
                        }
                        *current_track += 1;
                        *current_track - 1
                    };

                    let track = {
                        let playlist = playlist.lock().await;
                        playlist[index].clone()
                    };

                    match fetch_audio_url(&client, &track.bvid, &track.cid).await {
                        Ok(url) => player.set_uri(Some(&url)),
                        Err(e) => eprintln!("Error fetching audio URL: {:?}", e),
                    }
                    player.play();
                }
            });
        }

        // 保持主线程运行以便播放音频
        let main_loop = MainLoop::new(None, false);
        main_loop.run();

        Ok(())
    }
}
