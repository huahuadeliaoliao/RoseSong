use crate::error::ApplicationError;
use crate::player::network::{fetch_and_verify_audio_url, set_pipeline_uri_with_headers};
use crate::player::playlist::{
    get_current_track, load_playlist, move_to_next_track, move_to_previous_track,
    set_current_track_index, PlayMode, CURRENT_TRACK_INDEX, PLAYLIST,
};
use futures_util::stream::StreamExt;
use gstreamer::prelude::*;
use gstreamer::MessageView;
use gstreamer::Pipeline;
use log::{error, info};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task;

pub enum PlayerCommand {
    Play,
    PlayBvid(String),
    Pause,
    Next,
    Previous,
    Stop,
    SetPlayMode(PlayMode),
    ReloadPlaylist,
    PlaylistIsEmpty,
}

#[derive(Clone, Debug)]
pub struct AudioPlayer {
    pipeline: Arc<Pipeline>,
    client: Arc<Client>,
    play_mode: Arc<RwLock<PlayMode>>,
    command_receiver: Arc<Mutex<mpsc::Receiver<PlayerCommand>>>,
    eos_sender: mpsc::Sender<()>,
}

impl AudioPlayer {
    pub async fn new(
        play_mode: PlayMode,
        initial_track_index: usize,
        command_receiver: Arc<Mutex<mpsc::Receiver<PlayerCommand>>>,
    ) -> Result<Self, ApplicationError> {
        gstreamer::init().map_err(|e| ApplicationError::InitError(e.to_string()))?;
        let pipeline = Arc::new(gstreamer::Pipeline::new());
        let client = Arc::new(Client::new());
        set_current_track_index(initial_track_index).await?;
        let (eos_sender, eos_receiver) = mpsc::channel(1);

        info!("GStreamer created successfully.");
        let audio_player = Self {
            pipeline,
            client,
            play_mode: Arc::new(RwLock::new(play_mode)),
            command_receiver,
            eos_sender,
        };

        audio_player.start_eos_listener(eos_receiver).await?;

        Ok(audio_player)
    }

    async fn start_eos_listener(
        &self,
        mut eos_receiver: mpsc::Receiver<()>,
    ) -> Result<(), ApplicationError> {
        let pipeline = Arc::clone(&self.pipeline);
        let client = Arc::clone(&self.client);
        let play_mode = Arc::clone(&self.play_mode);

        task::spawn(async move {
            while let Some(_) = eos_receiver.recv().await {
                info!("Track finished playing. Handling EOS...");

                let current_play_mode = *play_mode.read().await;
                if current_play_mode != PlayMode::Repeat {
                    if let Err(e) = move_to_next_track(current_play_mode).await {
                        error!("Error moving to next track: {}", e);
                        continue;
                    }
                }

                if let Err(e) = play_track(&pipeline, &client).await {
                    error!("Failed to play next track: {}", e);
                }
            }
        });

        Ok(())
    }

    pub async fn play_playlist(&self) -> Result<(), ApplicationError> {
        let pipeline = Arc::clone(&self.pipeline);
        let client = Arc::clone(&self.client);
        let play_mode = Arc::clone(&self.play_mode);
        let command_receiver = Arc::clone(&self.command_receiver);
        let eos_sender = self.eos_sender.clone();

        // Watch GStreamer bus messages
        let bus = self.pipeline.bus().ok_or_else(|| {
            ApplicationError::PipelineError("Failed to get GStreamer bus".to_string())
        })?;

        let bus_receiver = bus.stream().for_each(move |msg| {
            let eos_sender = eos_sender.clone();
            async move {
                match msg.view() {
                    MessageView::Eos(_) => {
                        info!("EOS message received, sending signal.");
                        if eos_sender.send(()).await.is_err() {
                            error!("Failed to send EOS signal");
                        }
                    }
                    MessageView::Error(err) => {
                        error!("Error from GStreamer pipeline: {}", err);
                    }
                    _ => (),
                }
            }
        });

        // Listen for commands and process GStreamer messages concurrently
        task::spawn(async move {
            let mut command_receiver = command_receiver.lock().await;
            tokio::pin!(bus_receiver);
            loop {
                tokio::select! {
                    command = command_receiver.recv() => {
                        if let Some(command) = command {
                            match command {
                                PlayerCommand::Play => {
                                    info!("Resume playback");
                                    if let Err(e) = pipeline.set_state(gstreamer::State::Playing) {
                                        error!("Failed to play: {}", e);
                                    }
                                }
                                PlayerCommand::PlayBvid(new_bvid) => {
                                    info!("Play {}", new_bvid);
                                    {
                                        let playlist = PLAYLIST.lock().await;
                                        let playlist = playlist.as_ref().unwrap();

                                        if let Some(new_index) = playlist.find_track_index(&new_bvid).await {
                                            set_current_track_index(new_index).await.ok();
                                        } else {
                                            error!("Track with bvid {} not found in the playlist", new_bvid);
                                        }
                                    }
                                    if let Err(e) = play_track(&pipeline, &client).await {
                                        error!("Failed to play track after set new bvid: {}", e);
                                    }
                                }
                                PlayerCommand::Pause => {
                                    info!("Pause");
                                    if let Err(e) = pipeline.set_state(gstreamer::State::Paused) {
                                        error!("Failed to pause: {}", e);
                                    }
                                }
                                PlayerCommand::Next => {
                                    info!("Play next song");
                                    let current_play_mode = *play_mode.read().await;
                                    let mode = if current_play_mode == PlayMode::Repeat {
                                        PlayMode::Loop
                                    } else {
                                        current_play_mode
                                    };
                                    if let Err(e) = move_to_next_track(mode).await {
                                        error!("Failed to skip to next track: {}", e);
                                    } else if let Err(e) = play_track(&pipeline, &client).await {
                                        error!("Failed to play next track: {}", e);
                                    }
                                }
                                PlayerCommand::Previous => {
                                    info!("Play previous song");
                                    let current_play_mode = *play_mode.read().await;
                                    let mode = if current_play_mode == PlayMode::Repeat {
                                        PlayMode::Loop
                                    } else {
                                        current_play_mode
                                    };
                                    if let Err(e) = move_to_previous_track(mode).await {
                                        error!("Failed to skip to previous track: {}", e);
                                    } else if let Err(e) = play_track(&pipeline, &client).await {
                                        error!("Failed to play previous track: {}", e);
                                    }
                                }
                                PlayerCommand::Stop => {
                                    if let Err(e) = pipeline.set_state(gstreamer::State::Null) {
                                        error!("Failed to stop: {}", e);
                                    }
                                }
                                PlayerCommand::SetPlayMode(new_mode) => {
                                    let mut write_guard = play_mode.write().await;
                                    *write_guard = new_mode;
                                }
                                PlayerCommand::ReloadPlaylist => {
                                    // Get current track index and details
                                    let current_index = *CURRENT_TRACK_INDEX.lock().await;
                                    let current_track = get_current_track().await;

                                    // Load the new playlist
                                    if let Err(e) = load_playlist(&format!(
                                        "{}/.config/rosesong/playlists/playlist.toml",
                                        std::env::var("HOME").expect("Failed to get HOME environment variable")
                                    )).await {
                                        error!("Failed to reload playlist: {}", e);
                                    }

                                    // Handle the reloaded playlist
                                    let should_play = {
                                        let playlist = PLAYLIST.lock().await;
                                        let playlist = playlist.as_ref().unwrap();
                                        if let Ok(current_track) = current_track {
                                            // Find the index of the current track in the new playlist
                                            if let Some(new_index) = playlist.find_track_index(&current_track.bvid).await {
                                                set_current_track_index(new_index).await.ok();
                                                info!("Current track found in the new playlist, index set to {}", new_index);
                                                false // No need to play track again if it's found
                                            } else {
                                                // Track not found in the new playlist
                                                info!("Current track not found in the new playlist, resetting playback");
                                                let track_count = playlist.tracks.len();
                                                let new_index = if current_index < track_count {
                                                    current_index
                                                } else {
                                                    track_count - 1
                                                };
                                                set_current_track_index(new_index).await.ok();
                                                true
                                            }
                                        } else {
                                            false
                                        }
                                    };

                                    // Call play_track only if needed
                                    if should_play {
                                        if let Err(e) = play_track(&pipeline, &client).await {
                                            error!("Failed to play track after reloading playlist: {}", e);
                                        }
                                    }
                                }
                                PlayerCommand::PlaylistIsEmpty => {
                                    if let Err(e) = load_playlist(&format!(
                                        "{}/.config/rosesong/playlists/playlist.toml",
                                        std::env::var("HOME").expect("Failed to get HOME environment variable")
                                    )).await {
                                        error!("Failed to reload playlist: {}", e);
                                    }
                                    info!("set track");
                                    set_current_track_index(0).await.ok();
                                    if let Err(e) = play_track(&pipeline, &client).await {
                                        error!("Failed to play track after reloading playlist: {}", e);
                                    }
                                }
                            }
                        }
                    },
                    _ = &mut bus_receiver => {},
                }
            }
        });

        play_track(&self.pipeline, &self.client).await?;
        Ok(())
    }
}

async fn play_track(pipeline: &Pipeline, client: &Client) -> Result<(), ApplicationError> {
    pipeline
        .set_state(gstreamer::State::Null)
        .map_err(|_| ApplicationError::StateError("Failed to set pipeline to Null".to_string()))?;

    for element in pipeline.children() {
        pipeline.remove(&element).map_err(|_| {
            ApplicationError::ElementError("Failed to remove element from pipeline".to_string())
        })?;
    }

    pipeline
        .set_state(gstreamer::State::Ready)
        .map_err(|_| ApplicationError::StateError("Failed to set pipeline to Ready".to_string()))?;

    let track = get_current_track().await?;
    let url = fetch_and_verify_audio_url(client, &track.bvid, &track.cid).await?;

    set_pipeline_uri_with_headers(pipeline, &url).await?;

    pipeline.set_state(gstreamer::State::Playing).map_err(|_| {
        ApplicationError::StateError("Failed to set pipeline to Playing".to_string())
    })?;
    Ok(())
}
