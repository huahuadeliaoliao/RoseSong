use crate::error::ApplicationError;
use rand::seq::IteratorRandom;
use serde::Deserialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::LazyLock;
use tokio::sync::RwLock;

#[derive(Deserialize, Clone, Debug)]
pub struct Track {
    pub bvid: String,
    pub cid: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Playlist {
    pub tracks: Vec<Track>,
}

impl Playlist {
    pub async fn load_from_file(file_path: &str) -> Result<Self, ApplicationError> {
        log::info!("Loading playlist");
        let content = tokio::fs::read_to_string(file_path).await?;
        let playlist: Playlist = toml::from_str(&content)?;
        Ok(playlist)
    }

    pub async fn get_current_track(&self, index: usize) -> Result<Track, ApplicationError> {
        self.tracks
            .get(index)
            .cloned() // Clone the Track to return an owned value
            .ok_or_else(|| {
                ApplicationError::DataParsingError("Track index out of bounds".to_string())
            })
    }

    pub async fn move_to_next_track(
        &mut self,
        play_mode: PlayMode,
    ) -> Result<usize, ApplicationError> {
        let current_index = CURRENT_TRACK_INDEX.load(Ordering::SeqCst);
        let new_index = match play_mode {
            PlayMode::Loop => (current_index + 1) % self.tracks.len(),
            PlayMode::Shuffle => {
                let mut rng = rand::thread_rng();
                (0..self.tracks.len()).choose(&mut rng).ok_or_else(|| {
                    ApplicationError::DataParsingError("Failed to choose random track".to_string())
                })?
            }
            PlayMode::Repeat => current_index,
        };
        CURRENT_TRACK_INDEX.store(new_index, Ordering::SeqCst);
        Ok(new_index)
    }

    pub async fn move_to_previous_track(
        &mut self,
        play_mode: PlayMode,
    ) -> Result<usize, ApplicationError> {
        let current_index = CURRENT_TRACK_INDEX.load(Ordering::SeqCst);
        let new_index = match play_mode {
            PlayMode::Loop => {
                if current_index == 0 {
                    self.tracks.len() - 1
                } else {
                    current_index - 1
                }
            }
            PlayMode::Shuffle => {
                let mut rng = rand::thread_rng();
                (0..self.tracks.len()).choose(&mut rng).ok_or_else(|| {
                    ApplicationError::DataParsingError("Failed to choose random track".to_string())
                })?
            }
            PlayMode::Repeat => current_index,
        };
        CURRENT_TRACK_INDEX.store(new_index, Ordering::SeqCst);
        Ok(new_index)
    }

    pub async fn find_track_index(&self, bvid: &str) -> Option<usize> {
        self.tracks.iter().position(|track| track.bvid == bvid)
    }
}

pub static PLAYLIST: LazyLock<RwLock<Result<Playlist, ApplicationError>>> =
    LazyLock::new(|| RwLock::new(Ok(Playlist { tracks: Vec::new() })));
pub static CURRENT_TRACK_INDEX: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(0));

pub async fn load_playlist(file_path: &str) -> Result<(), ApplicationError> {
    let playlist = Playlist::load_from_file(file_path).await?;
    let mut playlist_lock = PLAYLIST.write().await;
    *playlist_lock = Ok(playlist); // Replace the old playlist with the new one
    Ok(())
}

pub async fn get_current_track() -> Result<Track, ApplicationError> {
    let playlist = PLAYLIST.read().await;
    let playlist = playlist.as_ref().map_err(|e| e.clone())?;
    let index = CURRENT_TRACK_INDEX.load(Ordering::SeqCst);
    playlist.get_current_track(index).await
}

pub async fn move_to_next_track(play_mode: PlayMode) -> Result<usize, ApplicationError> {
    let mut playlist = PLAYLIST.write().await;
    let playlist = playlist.as_mut().map_err(|e| e.clone())?;
    playlist.move_to_next_track(play_mode).await
}

pub async fn move_to_previous_track(play_mode: PlayMode) -> Result<usize, ApplicationError> {
    let mut playlist = PLAYLIST.write().await;
    let playlist = playlist.as_mut().map_err(|e| e.clone())?;
    playlist.move_to_previous_track(play_mode).await
}

pub async fn set_current_track_index(index: usize) -> Result<(), ApplicationError> {
    CURRENT_TRACK_INDEX.store(index, Ordering::SeqCst);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayMode {
    Loop,
    Shuffle,
    Repeat,
}
