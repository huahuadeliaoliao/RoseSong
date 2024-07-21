use crate::error::ApplicationError;
use crate::player::network::{fetch_and_verify_audio_url, set_pipeline_uri_with_headers};
use crate::player::playlist::{
    get_current_track, move_to_next_track, move_to_previous_track, set_current_track_index,
    PlayMode,
};
use futures_util::stream::StreamExt;
use gstreamer::prelude::*;
use gstreamer::MessageView;
use gstreamer::Pipeline;
use log::{error, info};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::task;

#[derive(Clone, Debug)]
pub enum PlayerCommand {
    Play,
    Pause,
    Next,
    Previous,
    Stop,
}

#[derive(Clone, Debug)]
pub struct AudioPlayer {
    pipeline: Arc<Pipeline>,
    client: Arc<Client>,
    play_mode: PlayMode,
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
        set_current_track_index(initial_track_index)?;
        let (eos_sender, eos_receiver) = mpsc::channel(1);

        info!("GStreamer created successfully.");
        let audio_player = Self {
            pipeline,
            client,
            play_mode,
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
        let play_mode = self.play_mode;

        task::spawn(async move {
            while let Some(_) = eos_receiver.recv().await {
                info!("Track finished playing. Handling EOS...");

                if play_mode != PlayMode::SingleRepeat {
                    if let Err(e) = move_to_next_track(play_mode) {
                        error!("Error moving to next track: {}", e);
                        continue;
                    }
                }

                if let Err(e) = play_next_track(&pipeline, &client).await {
                    error!("Failed to play next track: {}", e);
                }
            }
        });

        Ok(())
    }

    pub async fn play_playlist(&self) -> Result<(), ApplicationError> {
        let pipeline = Arc::clone(&self.pipeline);
        let client = Arc::clone(&self.client);
        let play_mode = self.play_mode;
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
                                    if let Err(e) = pipeline.set_state(gstreamer::State::Playing) {
                                        error!("Failed to play: {}", e);
                                    }
                                }
                                PlayerCommand::Pause => {
                                    if let Err(e) = pipeline.set_state(gstreamer::State::Paused) {
                                        error!("Failed to pause: {}", e);
                                    }
                                }
                                PlayerCommand::Next => {
                                    let mode = if play_mode == PlayMode::SingleRepeat {
                                        PlayMode::Loop
                                    } else {
                                        play_mode
                                    };
                                    if let Err(e) = move_to_next_track(mode) {
                                        error!("Failed to skip to next track: {}", e);
                                    } else if let Err(e) = play_next_track(&pipeline, &client).await {
                                        error!("Failed to play next track: {}", e);
                                    }
                                }
                                PlayerCommand::Previous => {
                                    let mode = if play_mode == PlayMode::SingleRepeat {
                                        PlayMode::Loop
                                    } else {
                                        play_mode
                                    };
                                    if let Err(e) = move_to_previous_track(mode) {
                                        error!("Failed to skip to previous track: {}", e);
                                    } else if let Err(e) = play_next_track(&pipeline, &client).await {
                                        error!("Failed to play previous track: {}", e);
                                    }
                                }
                                PlayerCommand::Stop => {
                                    if let Err(e) = pipeline.set_state(gstreamer::State::Null) {
                                        error!("Failed to stop: {}", e);
                                    }
                                }
                            }
                        }
                    },
                    _ = &mut bus_receiver => {},
                }
            }
        });

        play_next_track(&self.pipeline, &self.client).await?;
        Ok(())
    }
}

async fn play_next_track(pipeline: &Pipeline, client: &Client) -> Result<(), ApplicationError> {
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

    let track = get_current_track()?;
    let url = fetch_and_verify_audio_url(client, &track.bvid, &track.cid).await?;

    set_pipeline_uri_with_headers(pipeline, &url).await?;

    pipeline.set_state(gstreamer::State::Playing).map_err(|_| {
        ApplicationError::StateError("Failed to set pipeline to Playing".to_string())
    })?;
    Ok(())
}
