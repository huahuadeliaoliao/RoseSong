use crate::error::ApplicationError;
use lazy_static::lazy_static;
use rand::seq::IteratorRandom;
use serde::Deserialize;
use std::fs;
use std::process;
use std::sync::Mutex;

#[derive(Deserialize, Clone, Debug)]
pub struct Track {
    pub bvid: String,
    pub cid: String,
    pub title: Option<String>,
    pub owner: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Playlist {
    pub tracks: Vec<Track>,
}

impl Playlist {
    pub fn load_from_file(file_path: &str) -> Result<Self, ApplicationError> {
        log::info!("Loading playlist from file: {}", file_path);
        let content = fs::read_to_string(file_path)?;
        if content.trim().is_empty() {
            log::error!("Current playlist is empty");
            process::exit(1);
        }
        let playlist: Playlist = toml::from_str(&content)?;
        Ok(playlist)
    }

    pub fn get_current_track(&self, index: usize) -> Result<&Track, ApplicationError> {
        self.tracks.get(index).ok_or_else(|| {
            ApplicationError::DataParsingError("Track index out of bounds".to_string())
        })
    }

    pub fn move_to_next_track(&mut self, play_mode: PlayMode) -> Result<usize, ApplicationError> {
        let mut current_index = CURRENT_TRACK_INDEX
            .lock()
            .map_err(|e| ApplicationError::MutexLockError(e.to_string()))?;
        match play_mode {
            PlayMode::Loop => {
                *current_index = (*current_index + 1) % self.tracks.len();
            }
            PlayMode::Shuffle => {
                let mut rng = rand::thread_rng();
                *current_index = (0..self.tracks.len()).choose(&mut rng).ok_or_else(|| {
                    ApplicationError::DataParsingError("Failed to choose random track".to_string())
                })?;
            }
            PlayMode::SingleRepeat => {
                // Do nothing, keep the current index
            }
        }
        Ok(*current_index)
    }
}

lazy_static! {
    pub static ref PLAYLIST: Mutex<Result<Playlist, ApplicationError>> =
        Mutex::new(Playlist::load_from_file(&format!(
            "{}/.config/rosesong/playlists/playlist.toml",
            std::env::var("HOME").expect("Failed to get HOME environment variable")
        )));
    pub static ref CURRENT_TRACK_INDEX: Mutex<usize> = Mutex::new(0);
}

pub fn get_current_track() -> Result<Track, ApplicationError> {
    let playlist = PLAYLIST
        .lock()
        .map_err(|e| ApplicationError::MutexLockError(e.to_string()))?;
    let playlist = playlist.as_ref().map_err(|e| e.clone())?;
    let index = *CURRENT_TRACK_INDEX
        .lock()
        .map_err(|e| ApplicationError::MutexLockError(e.to_string()))?;
    playlist.get_current_track(index).cloned()
}

pub fn move_to_next_track(play_mode: PlayMode) -> Result<usize, ApplicationError> {
    let mut playlist = PLAYLIST
        .lock()
        .map_err(|e| ApplicationError::MutexLockError(e.to_string()))?;
    let playlist = playlist.as_mut().map_err(|e| e.clone())?;
    playlist.move_to_next_track(play_mode)
}

pub fn set_current_track_index(index: usize) -> Result<(), ApplicationError> {
    let mut current_index = CURRENT_TRACK_INDEX
        .lock()
        .map_err(|e| ApplicationError::MutexLockError(e.to_string()))?;
    *current_index = index;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayMode {
    Loop,
    Shuffle,
    SingleRepeat,
}
