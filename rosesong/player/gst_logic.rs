use crate::error::App;
use crate::player::network::{fetch_and_verify_audio_url, set_pipeline_uri_with_headers};
use crate::player::playlist::{
    get_current_track, load, move_to_next_track, move_to_previous_track, set_current_track_index,
    PlayMode, CURRENT_TRACK_INDEX, PLAYLIST,
};
use futures_util::stream::StreamExt;
use gstreamer::prelude::*;
use gstreamer::MessageView;
use gstreamer::Pipeline;
use log::{error, info};
use reqwest::Client;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task;

pub enum Command {
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
pub struct Audio {
    pipeline: Arc<Pipeline>,
    client: Arc<Client>,
    play_mode: Arc<RwLock<PlayMode>>,
    command_receiver: Arc<Mutex<mpsc::Receiver<Command>>>,
    eos_sender: mpsc::Sender<()>,
}

impl Audio {
    pub async fn new(
        play_mode: PlayMode,
        initial_track_index: usize,
        command_receiver: Arc<Mutex<mpsc::Receiver<Command>>>,
    ) -> Result<Self, App> {
        gstreamer::init().map_err(|e| App::Init(e.to_string()))?;
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

        audio_player.start_eos_listener(eos_receiver);

        Ok(audio_player)
    }

    fn start_eos_listener(&self, mut eos_receiver: mpsc::Receiver<()>) {
        let pipeline = Arc::clone(&self.pipeline);
        let client = Arc::clone(&self.client);
        let play_mode = Arc::clone(&self.play_mode);

        task::spawn(async move {
            while let Some(()) = eos_receiver.recv().await {
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
    }

    pub async fn play_playlist(&self) -> Result<(), App> {
        let pipeline = Arc::clone(&self.pipeline);
        let client = Arc::clone(&self.client);
        let play_mode = Arc::clone(&self.play_mode);
        let command_receiver = Arc::clone(&self.command_receiver);
        let eos_sender = self.eos_sender.clone();

        self.listen_to_bus(&eos_sender.clone())?;
        Audio::listen_for_commands(command_receiver, pipeline, client, play_mode, &eos_sender);

        play_track(&self.pipeline, &self.client).await?;
        Ok(())
    }

    fn listen_to_bus(&self, eos_sender: &mpsc::Sender<()>) -> Result<(), App> {
        let bus = self
            .pipeline
            .bus()
            .ok_or_else(|| App::Pipeline("Failed to get GStreamer bus".to_string()))?;

        task::spawn({
            let eos_sender = eos_sender.clone();
            bus.stream().for_each(move |msg| {
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
            })
        });
        Ok(())
    }

    fn listen_for_commands(
        command_receiver: Arc<Mutex<mpsc::Receiver<Command>>>,
        pipeline: Arc<Pipeline>,
        client: Arc<Client>,
        play_mode: Arc<RwLock<PlayMode>>,
        _eos_sender: &mpsc::Sender<()>,
    ) {
        task::spawn(async move {
            let mut command_receiver = command_receiver.lock().await;
            loop {
                if let Some(command) = command_receiver.recv().await {
                    match command {
                        Command::Play => {
                            info!("Resume playback");
                            if let Err(e) = pipeline.set_state(gstreamer::State::Playing) {
                                error!("Failed to play: {}", e);
                            }
                        }
                        Command::PlayBvid(new_bvid) => {
                            info!("Play {}", new_bvid);
                            if let Err(e) = handle_play_bvid(&new_bvid, &pipeline, &client).await {
                                error!("Failed to play track: {}", e);
                            }
                        }
                        Command::Pause => {
                            info!("Pause");
                            if let Err(e) = pipeline.set_state(gstreamer::State::Paused) {
                                error!("Failed to pause: {}", e);
                            }
                        }
                        Command::Next => {
                            info!("Play next song");
                            if let Err(e) =
                                handle_next_track(play_mode.clone(), &pipeline, &client).await
                            {
                                error!("Failed to play next track: {}", e);
                            }
                        }
                        Command::Previous => {
                            info!("Play previous song");
                            if let Err(e) =
                                handle_previous_track(play_mode.clone(), &pipeline, &client).await
                            {
                                error!("Failed to play previous track: {}", e);
                            }
                        }
                        Command::Stop => {
                            if let Err(e) = pipeline.set_state(gstreamer::State::Null) {
                                error!("Failed to stop: {}", e);
                            }
                        }
                        Command::SetPlayMode(new_mode) => {
                            let mut write_guard = play_mode.write().await;
                            *write_guard = new_mode;
                        }
                        Command::ReloadPlaylist => {
                            if let Err(e) = handle_reload_playlist().await {
                                error!("Failed to reload playlist: {}", e);
                            }
                        }
                        Command::PlaylistIsEmpty => {
                            if let Err(e) = handle_playlist_is_empty(&pipeline, &client).await {
                                error!("Failed to play track after reloading playlist: {}", e);
                            }
                        }
                    }
                }
            }
        });
    }
}

async fn handle_play_bvid(new_bvid: &str, pipeline: &Pipeline, client: &Client) -> Result<(), App> {
    let new_index;
    {
        let playlist = PLAYLIST.read().await;
        let playlist = playlist.as_ref().unwrap();
        new_index = playlist.find_track_index(new_bvid);
    }

    if let Some(index) = new_index {
        set_current_track_index(index).await.ok();
    } else {
        error!("Track with bvid {} not found in the playlist", new_bvid);
    }

    play_track(pipeline, client).await
}

async fn handle_next_track(
    play_mode: Arc<RwLock<PlayMode>>,
    pipeline: &Pipeline,
    client: &Client,
) -> Result<(), App> {
    let current_play_mode = *play_mode.read().await;
    let mode = if current_play_mode == PlayMode::Repeat {
        PlayMode::Loop
    } else {
        current_play_mode
    };
    move_to_next_track(mode).await?;
    play_track(pipeline, client).await
}

async fn handle_previous_track(
    play_mode: Arc<RwLock<PlayMode>>,
    pipeline: &Pipeline,
    client: &Client,
) -> Result<(), App> {
    let current_play_mode = *play_mode.read().await;
    let mode = if current_play_mode == PlayMode::Repeat {
        PlayMode::Loop
    } else {
        current_play_mode
    };
    move_to_previous_track(mode).await?;
    play_track(pipeline, client).await
}

async fn handle_reload_playlist() -> Result<(), App> {
    let current_index = CURRENT_TRACK_INDEX.load(Ordering::SeqCst);
    let current_track = get_current_track().await;

    load(&format!(
        "{}/.config/rosesong/playlists/playlist.toml",
        std::env::var("HOME").expect("Failed to get HOME environment variable")
    ))
    .await?;

    let should_play = {
        let playlist = PLAYLIST.read().await;
        let playlist = playlist.as_ref().unwrap();
        if let Ok(current_track) = current_track {
            if let Some(new_index) = playlist.find_track_index(&current_track.bvid) {
                set_current_track_index(new_index).await.ok();
                info!(
                    "Current track found in the new playlist, index set to {}",
                    new_index
                );
                false
            } else {
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

    if should_play {
        let pipeline = Arc::new(gstreamer::Pipeline::new());
        let client = Arc::new(Client::new());
        play_track(&pipeline, &client).await?;
    }

    Ok(())
}

async fn handle_playlist_is_empty(pipeline: &Pipeline, client: &Client) -> Result<(), App> {
    load(&format!(
        "{}/.config/rosesong/playlists/playlist.toml",
        std::env::var("HOME").expect("Failed to get HOME environment variable")
    ))
    .await?;

    info!("Set track");
    set_current_track_index(0).await.ok();
    play_track(pipeline, client).await
}

async fn play_track(pipeline: &Pipeline, client: &Client) -> Result<(), App> {
    pipeline
        .set_state(gstreamer::State::Null)
        .map_err(|_| App::State("Failed to set pipeline to Null".to_string()))?;

    for element in pipeline.children() {
        pipeline
            .remove(&element)
            .map_err(|_| App::Element("Failed to remove element from pipeline".to_string()))?;
    }

    pipeline
        .set_state(gstreamer::State::Ready)
        .map_err(|_| App::State("Failed to set pipeline to Ready".to_string()))?;

    let track = get_current_track().await?;
    let url = fetch_and_verify_audio_url(client, &track.bvid, &track.cid).await?;

    set_pipeline_uri_with_headers(pipeline, &url).await?;

    pipeline
        .set_state(gstreamer::State::Playing)
        .map_err(|_| App::State("Failed to set pipeline to Playing".to_string()))?;
    Ok(())
}
