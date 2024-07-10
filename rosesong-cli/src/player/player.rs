use crate::bilibili::fetch_audio_url::fetch_audio_url;
use crate::error::ApplicationError;
use crate::player::playlist::{
    get_current_track, move_to_next_track, set_current_track_index, PlayMode,
};
use glib::object::ObjectExt;
use glib::ControlFlow;
use glib::MainLoop;
use gstreamer::prelude::*;
use gstreamer::{ClockTime, Pipeline};
use gstreamer_player::PlayerState;
use log::{error, info};
use reqwest::header::{ACCEPT, RANGE, USER_AGENT};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{mpsc, Notify, RwLock};
use tokio::time::{sleep, Duration};

#[derive(Clone)]
pub struct AudioPlayer {
    pipeline: Arc<Pipeline>,
    client: Arc<Client>,
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
        info!("Creating GStreamer pipeline...");
        let pipeline = Arc::new(gstreamer::Pipeline::new());

        let client = Arc::new(Client::new());
        let track_finished = Arc::new(Notify::new());
        let current_state = Arc::new(RwLock::new(PlayerState::Stopped));

        set_current_track_index(initial_track_index)?;

        let (sender, mut receiver) = mpsc::channel(32);

        {
            let pipeline = Arc::clone(&pipeline);
            let client = Arc::clone(&client);
            let current_state = Arc::clone(&current_state);
            let play_mode = play_mode.clone();

            tokio::spawn(async move {
                while let Some(command) = receiver.recv().await {
                    match command {
                        PlayerCommand::Play => {
                            let mut state = current_state.write().await;
                            *state = PlayerState::Playing;
                            pipeline.set_state(gstreamer::State::Playing).unwrap();
                        }
                        PlayerCommand::Pause => {
                            let mut state = current_state.write().await;
                            *state = PlayerState::Paused;
                            pipeline.set_state(gstreamer::State::Paused).unwrap();
                        }
                        PlayerCommand::PreviousTrack | PlayerCommand::NextTrack => {
                            let move_next = matches!(command, PlayerCommand::NextTrack);
                            if move_next {
                                move_to_next_track(play_mode.clone()).unwrap();
                            } else {
                                move_to_next_track(PlayMode::Loop).unwrap();
                            }
                            let track = get_current_track().unwrap();
                            match fetch_and_verify_audio_url(&client, &track.bvid, &track.cid).await
                            {
                                Ok(url) => {
                                    set_pipeline_uri_with_headers(&pipeline, &url).await;
                                    pipeline.set_state(gstreamer::State::Playing).unwrap();
                                }
                                Err(e) => {
                                    error!("Error fetching audio URL: {}", e);
                                }
                            }
                        }
                        PlayerCommand::SetPosition(position) => {
                            pipeline
                                .seek_simple(
                                    gstreamer::SeekFlags::FLUSH,
                                    ClockTime::from_nseconds(position),
                                )
                                .unwrap();
                        }
                        PlayerCommand::GetPosition(responder) => {
                            let position = pipeline
                                .query_position::<ClockTime>()
                                .map(|p| p.nseconds())
                                .unwrap_or(0);
                            let _ = responder.send(position);
                        }
                        PlayerCommand::GetDuration(responder) => {
                            let duration = pipeline
                                .query_duration::<ClockTime>()
                                .map(|d| d.nseconds())
                                .unwrap_or(0);
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
            pipeline,
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
                set_pipeline_uri_with_headers(&self.pipeline, &url).await;
                info!("Starting playback for track: {:?}", track);
                self.pipeline.set_state(gstreamer::State::Playing).unwrap();
                Ok(())
            }
            Err(e) => {
                error!("Error fetching and verifying audio URL");
                Err(ApplicationError::FetchError(e.to_string()))
            }
        }
    }

    pub async fn play_playlist(&self) -> Result<(), ApplicationError> {
        let pipeline_clone = Arc::clone(&self.pipeline);
        let client_clone = Arc::clone(&self.client);
        let play_mode = self.play_mode;

        // Create a channel to send messages to an async task
        let (sync_sender, mut sync_receiver) = mpsc::channel(32);

        // Use another clone for the async task
        let pipeline_async = Arc::clone(&self.pipeline);
        let client_async = Arc::clone(&self.client);

        // Spawn an async task to process the messages
        tokio::spawn(async move {
            while let Some(_msg) = sync_receiver.recv().await {
                if let Err(e) = play_next_track(&pipeline_async, &client_async).await {
                    error!("Failed to play next track: {}", e);
                }
            }
        });

        // Watch for GStreamer messages
        let sync_sender_clone = sync_sender.clone();
        let _bus_watch_guard = pipeline_clone
            .bus()
            .unwrap()
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

                        // Send a message to the async task to play the next track
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
            .unwrap();

        play_next_track(&pipeline_clone, &client_clone).await?;

        info!("Starting main loop to keep audio playing...");
        let main_loop = MainLoop::new(None, false);
        main_loop.run();

        info!("Main loop exited.");
        Ok(())
    }
}

async fn play_next_track(pipeline: &Pipeline, client: &Client) -> Result<(), ApplicationError> {
    // Stop the pipeline
    pipeline.set_state(gstreamer::State::Null).unwrap();

    // Remove all elements from the pipeline
    let elements = pipeline.children();
    for element in elements {
        pipeline.remove(&element).unwrap();
    }

    // Reset the pipeline state
    pipeline.set_state(gstreamer::State::Ready).unwrap();

    // Fetch and verify the audio URL
    let track = get_current_track()?;
    let url = fetch_and_verify_audio_url(client, &track.bvid, &track.cid).await?;

    // Reconfigure the pipeline with the new audio URL
    set_pipeline_uri_with_headers(pipeline, &url).await;

    // Set the pipeline state to playing
    pipeline.set_state(gstreamer::State::Playing).unwrap();
    Ok(())
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

async fn set_pipeline_uri_with_headers(pipeline: &Pipeline, url: &str) {
    let source = gstreamer::ElementFactory::make("souphttpsrc")
        .build()
        .expect("Failed to create souphttpsrc element");
    source.set_property("location", url);

    let mut headers = gstreamer::Structure::new_empty("headers");
    headers.set(
        "User-Agent",
        &"Mozilla/5.0 BiliDroid/..* (bbcallen@gmail.com)",
    );
    headers.set("Referer", &"https://www.bilibili.com");
    source.set_property("extra-headers", &headers);

    let decodebin = gstreamer::ElementFactory::make("decodebin")
        .build()
        .expect("Failed to create decodebin element");

    pipeline
        .add_many(&[&source, &decodebin])
        .expect("Failed to add elements to pipeline");
    source
        .link(&decodebin)
        .expect("Failed to link source to decodebin");

    let pipeline_weak = pipeline.downgrade();

    decodebin.connect_pad_added(move |_, src_pad| {
        if let Some(pipeline) = pipeline_weak.upgrade() {
            info!("Pad {} added to decodebin", src_pad.name());

            let audioconvert = gstreamer::ElementFactory::make("audioconvert")
                .build()
                .expect("Failed to create audioconvert element");
            let audioresample = gstreamer::ElementFactory::make("audioresample")
                .build()
                .expect("Failed to create audioresample element");
            let autoaudiosink = gstreamer::ElementFactory::make("autoaudiosink")
                .build()
                .expect("Failed to create autoaudiosink element");

            pipeline
                .add_many(&[&audioconvert, &audioresample, &autoaudiosink])
                .expect("Failed to add elements to pipeline");

            audioconvert
                .sync_state_with_parent()
                .expect("Failed to sync_state_with_parent for audioconvert");
            audioresample
                .sync_state_with_parent()
                .expect("Failed to sync_state_with_parent for audioresample");
            autoaudiosink
                .sync_state_with_parent()
                .expect("Failed to sync_state_with_parent for autoaudiosink");

            let audio_pad = audioconvert
                .static_pad("sink")
                .expect("Failed to get static pad");
            src_pad.link(&audio_pad).expect("Failed to link pads");

            audioconvert
                .link(&audioresample)
                .expect("Failed to link audioconvert to audioresample");
            audioresample
                .link(&autoaudiosink)
                .expect("Failed to link audioresample to autoaudiosink");

            info!("Pipeline elements linked successfully");
        } else {
            error!("Failed to upgrade pipeline reference");
        }
    });

    pipeline.set_state(gstreamer::State::Playing).unwrap();
}
