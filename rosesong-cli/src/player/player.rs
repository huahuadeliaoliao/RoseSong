use crate::bilibili::fetch_audio_url::fetch_audio_url;
use crate::error::ApplicationError;
use crate::player::playlist::{
    get_current_track, move_to_next_track, set_current_track_index, PlayMode,
};
use glib::object::ObjectExt;
use glib::ControlFlow;
use glib::MainLoop;
use gstreamer::prelude::*;
use gstreamer::Pipeline;
use log::{error, info};
use reqwest::header::{ACCEPT, RANGE, USER_AGENT};
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::{mpsc, Notify, RwLock};
use tokio::time::{sleep, Duration};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
    Error,
}

#[derive(Clone)]
pub struct AudioPlayer {
    pipeline: Arc<Pipeline>,
    client: Arc<Client>,
    track_finished: Arc<Notify>,
    current_state: Arc<RwLock<PlayerState>>,
    play_mode: PlayMode,
}

impl AudioPlayer {
    pub async fn new(
        play_mode: PlayMode,
        initial_track_index: usize,
    ) -> Result<Self, ApplicationError> {
        gstreamer::init().map_err(|e| ApplicationError::InitError(e.to_string()))?;
        info!("Creating GStreamer pipeline...");
        let pipeline = Arc::new(gstreamer::Pipeline::new());

        let client = Arc::new(Client::new());
        let track_finished = Arc::new(Notify::new());
        let current_state = Arc::new(RwLock::new(PlayerState::Stopped));

        set_current_track_index(initial_track_index)?;

        info!("GStreamer created successfully.");
        Ok(Self {
            pipeline,
            client,
            track_finished,
            current_state,
            play_mode,
        })
    }

    pub async fn play_playlist(&self) -> Result<(), ApplicationError> {
        let (sync_sender, mut sync_receiver) = mpsc::channel(32);
        let pipeline = Arc::clone(&self.pipeline);
        let client = Arc::clone(&self.client);
        let play_mode = self.play_mode;

        // Spawn a task to handle next track playing
        tokio::spawn(async move {
            while sync_receiver.recv().await.is_some() {
                if let Err(e) = play_next_track(&pipeline, &client).await {
                    error!("Failed to play next track: {}", e);
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

async fn verify_audio_url(client: &Client, url: &str) -> Result<bool, ApplicationError> {
    let response = client
        .get(url)
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
        match fetch_audio_url(client, bvid, cid).await {
            Ok(url) => {
                if verify_audio_url(client, &url).await? {
                    return Ok(url);
                }
            }
            Err(_) => (),
        }
        if attempt < MAX_RETRIES {
            sleep(RETRY_DELAY).await;
        }
    }

    Err(ApplicationError::FetchError(
        "Max retries reached for fetching and verifying audio URL".to_string(),
    ))
}

async fn set_pipeline_uri_with_headers(
    pipeline: &Pipeline,
    url: &str,
) -> Result<(), ApplicationError> {
    let source = gstreamer::ElementFactory::make("souphttpsrc")
        .build()
        .map_err(|_| {
            ApplicationError::ElementError("Failed to create souphttpsrc element".to_string())
        })?;
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
        .map_err(|_| {
            ApplicationError::ElementError("Failed to create decodebin element".to_string())
        })?;

    pipeline.add_many(&[&source, &decodebin]).map_err(|_| {
        ApplicationError::PipelineError("Failed to add elements to pipeline".to_string())
    })?;
    source.link(&decodebin).map_err(|_| {
        ApplicationError::LinkError("Failed to link source to decodebin".to_string())
    })?;

    let pipeline_weak = pipeline.downgrade();

    decodebin.connect_pad_added(move |_, src_pad| {
        if let Some(pipeline) = pipeline_weak.upgrade() {
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

    pipeline.set_state(gstreamer::State::Playing).map_err(|_| {
        ApplicationError::StateError("Failed to set pipeline to Playing".to_string())
    })?;
    Ok(())
}
