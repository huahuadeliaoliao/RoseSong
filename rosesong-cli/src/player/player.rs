// player.rs
use crate::error::ApplicationError;
use crate::player::network::{fetch_and_verify_audio_url, set_pipeline_uri_with_headers};
use crate::player::playlist::{
    get_current_track, move_to_next_track, move_to_previous_track, set_current_track_index,
    PlayMode,
};
use glib::{ControlFlow, MainLoop};
use gstreamer::prelude::*;
use gstreamer::Pipeline;
use log::{error, info};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayerState {
    Stopped,
}

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

        info!("GStreamer created successfully.");
        Ok(Self {
            pipeline,
            client,
            play_mode,
            command_receiver,
        })
    }

    pub async fn play_playlist(&self) -> Result<(), ApplicationError> {
        let (sync_sender, mut sync_receiver) = mpsc::channel(1);
        let pipeline = Arc::clone(&self.pipeline);
        let client = Arc::clone(&self.client);
        let play_mode = self.play_mode;
        let command_receiver = Arc::clone(&self.command_receiver);

        // Spawn a task to handle next track playing
        task::spawn({
            let pipeline = Arc::clone(&pipeline);
            let client = Arc::clone(&client);
            async move {
                while sync_receiver.recv().await.is_some() {
                    if let Err(e) = play_next_track(&pipeline, &client).await {
                        error!("Failed to play next track: {}", e);
                    }
                }
            }
        });

        // Watch GStreamer bus messages
        let sync_sender_clone = sync_sender.clone();
        let _bus_watch_guard = self
            .pipeline
            .bus()
            .ok_or_else(|| {
                ApplicationError::PipelineError("Failed to get GStreamer bus".to_string())
            })?
            .add_watch(move |_, msg| {
                use gstreamer::MessageView;
                match msg.view() {
                    MessageView::Eos(_) => {
                        info!("Track finished playing.");
                        if play_mode != PlayMode::SingleRepeat {
                            if let Err(e) = move_to_next_track(play_mode) {
                                error!("Error moving to next track: {}", e);
                                return ControlFlow::Break;
                            }
                        }
                        // Notify to play next track
                        let _ = sync_sender_clone.try_send(());
                    }
                    MessageView::Error(err) => {
                        error!("Error from GStreamer pipeline: {}", err);
                        return ControlFlow::Break;
                    }
                    _ => (),
                }
                ControlFlow::Continue
            })
            .map_err(|_| {
                ApplicationError::PipelineError("Failed to add watch to GStreamer bus".to_string())
            })?;

        // Listen for commands
        task::spawn({
            let pipeline = Arc::clone(&pipeline);
            let client = Arc::clone(&client);
            async move {
                let mut command_receiver = command_receiver.lock().await;
                while let Some(command) = command_receiver.recv().await {
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
                            if let Err(e) = move_to_next_track(play_mode) {
                                error!("Failed to skip to next track: {}", e);
                            } else if let Err(e) = play_next_track(&pipeline, &client).await {
                                error!("Failed to play next track: {}", e);
                            }
                        }
                        PlayerCommand::Previous => {
                            if let Err(e) = move_to_previous_track(play_mode) {
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
            }
        });

        play_next_track(&self.pipeline, &self.client).await?;

        info!("Starting main loop to keep audio playing...");
        let main_loop = MainLoop::new(None, false);
        main_loop.run();

        info!("Main loop exited.");
        Ok(())
    }
}

async fn play_next_track(pipeline: &Pipeline, client: &Client) -> Result<(), ApplicationError> {
    pipeline
        .set_state(gstreamer::State::Null)
        .map_err(|_| ApplicationError::StateError("Failed to set pipeline to Null".to_string()))?;

    // Remove all elements from the pipeline
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
