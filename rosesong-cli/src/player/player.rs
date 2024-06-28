use crate::bilibili::fetch_audio_url::fetch_audio_url;
use crate::error::ApplicationError;
use crate::player::playlist::{Playlist, Track};
use glib::MainLoop;
use gstreamer::ClockTime;
use gstreamer_player::{
    Player, PlayerGMainContextSignalDispatcher, PlayerState, PlayerVideoRenderer,
};
use log::{error, info};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify, RwLock};

pub struct AudioPlayer {
    player: Player,
    client: Client,
    playlist: Arc<RwLock<Vec<Track>>>,
    current_track: Arc<Mutex<usize>>,
    track_finished: Arc<Notify>,
    current_state: Arc<Mutex<PlayerState>>,
}

impl AudioPlayer {
    pub async fn new(playlist_path: &str) -> Result<Self, ApplicationError> {
        // 初始化GStreamer
        info!("Initializing GStreamer...");
        gstreamer::init().map_err(|e| ApplicationError::InitError(e.to_string()))?;
        // 创建一个GStreamer Player
        info!("Creating GStreamer Player...");
        let player = Player::new(
            None::<PlayerVideoRenderer>,
            Some(PlayerGMainContextSignalDispatcher::new(None)),
        );

        let client = Client::new();

        // 加载播放列表
        info!("Loading playlist from file: {}", playlist_path);
        let playlist = Playlist::load_from_file(playlist_path)?;
        let playlist = Arc::new(RwLock::new(playlist.tracks));
        let current_track = Arc::new(Mutex::new(0));
        let track_finished = Arc::new(Notify::new());
        let current_state = Arc::new(Mutex::new(PlayerState::Stopped)); // 初始化播放状态

        // 连接 state_changed 信号
        {
            let current_state = Arc::clone(&current_state);
            player.connect_state_changed(move |_, state| {
                let current_state = Arc::clone(&current_state);
                tokio::spawn(async move {
                    let mut state_guard = current_state.lock().await;
                    *state_guard = state;
                });
            });
        }

        info!("AudioPlayer initialized successfully.");
        Ok(Self {
            player,
            client,
            playlist,
            current_track,
            track_finished,
            current_state,
        })
    }

    pub async fn play(&self) -> Result<(), ApplicationError> {
        info!("Playing track...");
        self.player.play();
        Ok(())
    }

    pub async fn pause(&self) -> Result<(), ApplicationError> {
        info!("Pausing track...");
        self.player.pause();
        Ok(())
    }

    pub async fn previous_track(&self) -> Result<(), ApplicationError> {
        info!("Switching to previous track...");
        let mut current_track = self.current_track.lock().await;
        if *current_track > 0 {
            *current_track -= 1;
            self.play_current_track().await?;
        }
        Ok(())
    }

    pub async fn next_track(&self) -> Result<(), ApplicationError> {
        info!("Switching to next track...");
        let mut current_track = self.current_track.lock().await;
        let playlist = self.playlist.read().await;
        if *current_track < playlist.len() - 1 {
            *current_track += 1;
            self.play_current_track().await?;
        }
        Ok(())
    }

    pub async fn set_position(&self, position: u64) -> Result<(), ApplicationError> {
        info!("Setting track position to {} nanoseconds...", position);
        self.player.seek(ClockTime::from_nseconds(position));
        Ok(())
    }

    pub async fn get_position(&self) -> Result<u64, ApplicationError> {
        let position = self.player.position().map(|p| p.nseconds()).unwrap_or(0);
        info!("Current track position: {} nanoseconds", position);
        Ok(position)
    }

    pub async fn get_playback_state(&self) -> Result<PlayerState, ApplicationError> {
        let state = self.current_state.lock().await;
        Ok(*state)
    }

    async fn play_current_track(&self) -> Result<(), ApplicationError> {
        let current_track = self.current_track.lock().await;
        let playlist = self.playlist.read().await;
        let track = &playlist[*current_track];

        match fetch_audio_url(&self.client, &track.bvid, &track.cid).await {
            Ok(url) => {
                info!("Fetched audio URL");
                self.player.set_uri(Some(&url));
                info!("Starting playback for track: {:?}", track);
                self.player.play();
                Ok(())
            }
            Err(e) => {
                error!("Error fetching audio URL");
                Err(ApplicationError::FetchError(e.to_string()))
            }
        }
    }

    pub async fn play_playlist(&self) -> Result<(), ApplicationError> {
        let playlist = Arc::clone(&self.playlist);
        let current_track = Arc::clone(&self.current_track);
        let player = self.player.clone();
        let client = self.client.clone();
        let track_finished = Arc::clone(&self.track_finished);

        player.connect_end_of_stream({
            let track_finished = Arc::clone(&track_finished);
            move |_| {
                info!("Track finished playing.");
                track_finished.notify_one();
            }
        });

        tokio::spawn(async move {
            loop {
                let mut current_track = current_track.lock().await;
                let playlist = playlist.read().await;
                if *current_track >= playlist.len() {
                    info!("Reached end of playlist.");
                    break;
                }
                let track = playlist[*current_track].clone();
                info!("Preparing to play track: {:?}", track);
                *current_track += 1;

                match fetch_audio_url(&client, &track.bvid, &track.cid).await {
                    Ok(url) => {
                        info!("Fetched audio URL");
                        player.set_uri(Some(&url));
                        info!("Starting playback for track: {:?}", track);
                        player.play();
                        track_finished.notified().await;
                    }
                    Err(e) => {
                        error!("Error fetching audio URL");
                    }
                }
            }
        });

        // 保持主线程运行以便播放音频
        info!("Starting main loop to keep audio playing...");
        let main_loop = MainLoop::new(None, false);
        main_loop.run();

        info!("Main loop exited.");
        Ok(())
    }
}
