use crate::bilibili::fetch_audio_url::fetch_audio_url;
use crate::error::ApplicationError;
use crate::player::playlist::{
    get_current_track, move_to_next_track, set_current_track_index, PlayMode,
};
use glib::MainLoop;
use gstreamer::ClockTime;
use gstreamer_player::{
    Player, PlayerGMainContextSignalDispatcher, PlayerState, PlayerVideoRenderer,
};
use log::{error, info};
use reqwest::header::{ACCEPT, RANGE, USER_AGENT};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{mpsc, Notify, RwLock};
use tokio::time::{sleep, Duration};

#[derive(Clone)]
pub struct AudioPlayer {
    player: Player,
    client: Client,
    track_finished: Arc<Notify>,
    current_state: Arc<RwLock<PlayerState>>,
    sender: mpsc::Sender<PlayerCommand>,
    play_mode: PlayMode,
}

enum PlayerCommand {
    Play,
    Pause,
    PreviousTrack,
    NextTrack,
    SetPosition(u64),
    GetPosition(mpsc::Sender<u64>),
    GetDuration(mpsc::Sender<u64>),
    GetPlaybackState(mpsc::Sender<PlayerState>),
}

impl AudioPlayer {
    pub async fn new(
        play_mode: PlayMode,
        initial_track_index: usize,
    ) -> Result<Self, ApplicationError> {
        info!("Initializing GStreamer...");
        gstreamer::init().map_err(|e| ApplicationError::InitError(e.to_string()))?;
        info!("Creating GStreamer Player...");
        let player = Player::new(
            None::<PlayerVideoRenderer>,
            Some(PlayerGMainContextSignalDispatcher::new(None)),
        );

        let client = Client::new();
        let track_finished = Arc::new(Notify::new());
        let current_state = Arc::new(RwLock::new(PlayerState::Stopped));

        set_current_track_index(initial_track_index);

        let (sender, mut receiver) = mpsc::channel(32);

        {
            let player = player.clone();
            let client = client.clone();
            let current_state = Arc::clone(&current_state);
            let play_mode = play_mode.clone();

            tokio::spawn(async move {
                while let Some(command) = receiver.recv().await {
                    match command {
                        PlayerCommand::Play => {
                            let mut state = current_state.write().await;
                            *state = PlayerState::Playing;
                            player.play();
                        }
                        PlayerCommand::Pause => {
                            let mut state = current_state.write().await;
                            *state = PlayerState::Paused;
                            player.pause();
                        }
                        PlayerCommand::PreviousTrack => {
                            move_to_next_track(PlayMode::Loop);
                            let track = get_current_track().unwrap();
                            match fetch_and_verify_audio_url(&client, &track.bvid, &track.cid).await
                            {
                                Ok(url) => {
                                    player.set_uri(Some(&url));
                                    player.play();
                                }
                                Err(e) => {
                                    error!("Error fetching audio URL: {}", e);
                                }
                            }
                        }
                        PlayerCommand::NextTrack => {
                            move_to_next_track(play_mode);
                            let track = get_current_track().unwrap();
                            match fetch_and_verify_audio_url(&client, &track.bvid, &track.cid).await
                            {
                                Ok(url) => {
                                    player.set_uri(Some(&url));
                                    player.play();
                                }
                                Err(e) => {
                                    error!("Error fetching audio URL: {}", e);
                                }
                            }
                        }
                        PlayerCommand::SetPosition(position) => {
                            player.seek(ClockTime::from_nseconds(position));
                        }
                        PlayerCommand::GetPosition(responder) => {
                            let position = player.position().map(|p| p.nseconds()).unwrap_or(0);
                            let _ = responder.send(position);
                        }
                        PlayerCommand::GetDuration(responder) => {
                            let duration = player.duration().map(|d| d.nseconds()).unwrap_or(0);
                            let _ = responder.send(duration);
                        }
                        PlayerCommand::GetPlaybackState(responder) => {
                            let state = *current_state.read().await;
                            let _ = responder.send(state);
                        }
                    }
                }
            });
        }

        info!("AudioPlayer initialized successfully.");
        Ok(Self {
            player,
            client,
            track_finished,
            current_state,
            sender,
            play_mode,
        })
    }

    pub async fn get_position(&self) -> Result<u64, ApplicationError> {
        let (responder, mut receiver) = mpsc::channel(1);
        self.sender
            .send(PlayerCommand::GetPosition(responder))
            .await?;
        receiver.recv().await.ok_or_else(|| {
            ApplicationError::DataParsingError("Failed to receive position".to_string())
        })
    }

    pub async fn get_duration(&self) -> Result<u64, ApplicationError> {
        let (responder, mut receiver) = mpsc::channel(1);
        self.sender
            .send(PlayerCommand::GetDuration(responder))
            .await?;
        receiver.recv().await.ok_or_else(|| {
            ApplicationError::DataParsingError("Failed to receive duration".to_string())
        })
    }

    pub async fn get_playback_state(&self) -> Result<PlayerState, ApplicationError> {
        let (responder, mut receiver) = mpsc::channel(1);
        self.sender
            .send(PlayerCommand::GetPlaybackState(responder))
            .await?;
        receiver.recv().await.ok_or_else(|| {
            ApplicationError::DataParsingError("Failed to receive playback state".to_string())
        })
    }

    pub async fn get_playback_info(&self) -> Result<(u64, u64), ApplicationError> {
        let position = self.get_position().await?;
        let duration = self.get_duration().await?;
        Ok((position, duration))
    }

    async fn play_current_track(&self) -> Result<(), ApplicationError> {
        let track = get_current_track()?;
        match fetch_and_verify_audio_url(&self.client, &track.bvid, &track.cid).await {
            Ok(url) => {
                info!("Fetched and verified audio URL");
                self.player.set_uri(Some(&url));
                info!("Starting playback for track: {:?}", track);
                self.player.play();
                Ok(())
            }
            Err(e) => {
                error!("Error fetching and verifying audio URL");
                Err(ApplicationError::FetchError(e.to_string()))
            }
        }
    }

    pub async fn play_playlist(&self) -> Result<(), ApplicationError> {
        let player = self.player.clone();
        let client = self.client.clone();
        let track_finished = Arc::clone(&self.track_finished);
        let play_mode = self.play_mode;

        player.connect_end_of_stream({
            let track_finished = Arc::clone(&track_finished);
            move |_| {
                info!("Track finished playing.");
                track_finished.notify_one();
            }
        });

        tokio::spawn(async move {
            loop {
                let track = match get_current_track() {
                    Ok(track) => track,
                    Err(e) => {
                        error!("Error getting current track: {}", e);
                        break;
                    }
                };

                match fetch_and_verify_audio_url(&client, &track.bvid, &track.cid).await {
                    Ok(url) => {
                        info!("Fetched and verified audio URL");
                        player.set_uri(Some(&url));
                        info!("Starting playback for track: {:?}", track);
                        player.play();
                        track_finished.notified().await;
                    }
                    Err(e) => {
                        error!("Error fetching and verifying audio URL: {}", e);
                        break;
                    }
                }

                if play_mode != PlayMode::SingleRepeat {
                    move_to_next_track(play_mode);
                }
            }
        });

        info!("Starting main loop to keep audio playing...");
        let main_loop = MainLoop::new(None, false);
        main_loop.run();

        info!("Main loop exited.");
        Ok(())
    }
}

async fn verify_audio_url(client: Arc<Client>, url: Arc<String>) -> Result<bool, ApplicationError> {
    let response = client
        .get(&*url)
        .header(USER_AGENT, "Mozilla/5.0 BiliDroid/..* (bbcallen@gmail.com)")
        .header(ACCEPT, "*/*")
        .header(RANGE, "bytes=0-1024")
        .header("Referer", "https://www.bilibili.com")
        .send()
        .await
        .map_err(|e| ApplicationError::NetworkError(e.to_string()))?;

    Ok(response.status().is_success())
}

async fn fetch_and_verify_audio_url(
    client: &Client,
    bvid: &str,
    cid: &str,
) -> Result<String, ApplicationError> {
    const MAX_RETRIES: u32 = 10;
    const RETRY_DELAY: Duration = Duration::from_secs(3);

    for attempt in 1..=MAX_RETRIES {
        let url = fetch_audio_url(client, bvid, cid).await?;
        if verify_audio_url(Arc::new(client.clone()), Arc::new(url.clone())).await? {
            return Ok(url);
        } else {
            error!(
                "Audio URL verification failed, attempt {}/{}",
                attempt, MAX_RETRIES
            );
        }

        if attempt < MAX_RETRIES {
            sleep(RETRY_DELAY).await;
        }
    }

    Err(ApplicationError::FetchError(
        "Max retries reached for fetching and verifying audio URL".to_string(),
    ))
}
