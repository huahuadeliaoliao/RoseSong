use crate::error::ApplicationError;
use lazy_static::lazy_static;
use rand::seq::IteratorRandom;
use serde::Deserialize;
use std::fs;
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
        let content = fs::read_to_string(file_path).map_err(|e| ApplicationError::IoError(e))?;
        let playlist: Playlist = toml::from_str(&content)
            .map_err(|e| ApplicationError::DataParsingError(e.to_string()))?;
        Ok(playlist)
    }

    pub fn get_current_track(&self, index: usize) -> Result<&Track, ApplicationError> {
        self.tracks.get(index).ok_or_else(|| {
            ApplicationError::DataParsingError("Track index out of bounds".to_string())
        })
    }

    pub fn move_to_next_track(&mut self, play_mode: PlayMode) -> usize {
        let mut current_index = CURRENT_TRACK_INDEX.lock().unwrap();
        match play_mode {
            PlayMode::Loop => {
                *current_index = (*current_index + 1) % self.tracks.len();
            }
            PlayMode::Shuffle => {
                let mut rng = rand::thread_rng();
                *current_index = (0..self.tracks.len()).choose(&mut rng).unwrap();
            }
            PlayMode::SingleRepeat => {
                // Do nothing, keep the current index
            }
        }
        *current_index
    }
}

lazy_static! {
    pub static ref PLAYLIST: Mutex<Playlist> =
        Mutex::new(Playlist::load_from_file("playlist.toml").expect("Failed to load playlist"));
    pub static ref CURRENT_TRACK_INDEX: Mutex<usize> = Mutex::new(0);
}

pub fn get_current_track() -> Result<Track, ApplicationError> {
    let playlist = PLAYLIST.lock().unwrap();
    let index = *CURRENT_TRACK_INDEX.lock().unwrap();
    playlist.get_current_track(index).cloned()
}

pub fn move_to_next_track(play_mode: PlayMode) -> usize {
    let mut playlist = PLAYLIST.lock().unwrap();
    playlist.move_to_next_track(play_mode)
}

pub fn set_current_track_index(index: usize) {
    let mut current_index = CURRENT_TRACK_INDEX.lock().unwrap();
    *current_index = index;
}

pub fn load_first_track_index(file_path: &str) -> Result<usize, ApplicationError> {
    let playlist = Playlist::load_from_file(file_path)?;
    if !playlist.tracks.is_empty() {
        Ok(0)
    } else {
        Err(ApplicationError::DataParsingError(
            "Playlist is empty".to_string(),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayMode {
    Loop,
    Shuffle,
    SingleRepeat,
}
