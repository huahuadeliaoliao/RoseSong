use crate::error::ApplicationError;
use crate::player::audio_url::fetch_audio_url;
use crate::player::playlist::{Playlist, Track};
use glib::MainLoop;
use gstreamer_player::{Player, PlayerGMainContextSignalDispatcher, PlayerVideoRenderer};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

pub struct AudioPlayer {
    player: Player,
    client: Client,
    playlist: Arc<Mutex<Vec<Track>>>,
    current_track: Arc<Mutex<usize>>,
    track_finished: Arc<Notify>,
}

impl AudioPlayer {
    pub async fn new(playlist_path: &str) -> Result<Self, ApplicationError> {
        // 初始化GStreamer
        println!("Initializing GStreamer...");
        gstreamer::init().map_err(|e| ApplicationError::InitError(e.to_string()))?;

        // 创建一个GStreamer Player
        println!("Creating GStreamer Player...");
        let player = Player::new(
            None::<PlayerVideoRenderer>,
            Some(PlayerGMainContextSignalDispatcher::new(None)),
        );

        let client = Client::new();

        // 加载播放列表
        println!("Loading playlist from file: {}", playlist_path);
        let playlist = Playlist::load_from_file(playlist_path)?;
        let playlist = Arc::new(Mutex::new(playlist.tracks));
        let current_track = Arc::new(Mutex::new(0));
        let track_finished = Arc::new(Notify::new());

        println!("AudioPlayer initialized successfully.");
        Ok(Self {
            player,
            client,
            playlist,
            current_track,
            track_finished,
        })
    }

    pub async fn play_playlist(&self) -> Result<(), ApplicationError> {
        {
            let playlist = Arc::clone(&self.playlist);
            let current_track = Arc::clone(&self.current_track);
            let player = self.player.clone();
            let client = self.client.clone();
            let track_finished = Arc::clone(&self.track_finished);

            player.connect_end_of_stream({
                let track_finished = Arc::clone(&track_finished);
                move |_| {
                    println!("Track finished playing.");
                    track_finished.notify_one();
                }
            });

            tokio::spawn(async move {
                loop {
                    let track = {
                        let mut current_track = current_track.lock().await;
                        if *current_track >= playlist.lock().await.len() {
                            println!("Reached end of playlist.");
                            break;
                        }
                        let track = playlist.lock().await[*current_track].clone();
                        println!("Preparing to play track: {:?}", track);
                        *current_track += 1;
                        track
                    };

                    match fetch_audio_url(&client, &track.bvid, &track.cid).await {
                        Ok(url) => {
                            println!("Fetched audio URL: {}", url);
                            player.set_uri(Some(&url));
                            println!("Starting playback for track: {:?}", track);
                            player.play();
                            track_finished.notified().await;
                        }
                        Err(e) => {
                            eprintln!("Error fetching audio URL: {:?}", e);
                        }
                    }
                }
            });
        }

        // 保持主线程运行以便播放音频
        println!("Starting main loop to keep audio playing...");
        let main_loop = MainLoop::new(None, false);
        main_loop.run();

        println!("Main loop exited.");
        Ok(())
    }
}
