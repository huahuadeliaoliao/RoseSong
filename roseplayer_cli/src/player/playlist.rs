use crate::error::ApplicationError;
use serde::Deserialize;
use std::fs;

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
    pub fn load_from_file(file_path: &str) -> Result<Self, ApplicationError> {
        let content = fs::read_to_string(file_path)?;
        let playlist: Playlist = toml::from_str(&content)?;
        Ok(playlist)
    }
}
