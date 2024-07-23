mod bilibili;
mod error;

use bilibili::fetch_audio_info::get_video_data;
use clap::{Parser, Subcommand};
use error::ApplicationError;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::{collections::HashSet, fs};
use zbus::{proxy, Connection};

type StdResult<T, E> = std::result::Result<T, E>;

#[proxy(
    interface = "org.rosesong.Player",
    default_service = "org.rosesong.Player",
    default_path = "/org/rosesong/Player"
)]
trait MyPlayer {
    async fn play(&self) -> zbus::Result<()>;
    async fn pause(&self) -> zbus::Result<()>;
    async fn next(&self) -> zbus::Result<()>;
    async fn previous(&self) -> zbus::Result<()>;
    async fn stop(&self) -> zbus::Result<()>;
    async fn set_mode(&self, mode: &str) -> zbus::Result<()>;
}

#[derive(Parser)]
#[command(name = "rsg", about = "Control the rosesong player.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Play,
    Pause,
    Next,
    Previous,
    Stop,
    Mode(ModeCommand),
    Import(ImportCommand),
}

#[derive(Parser)]
struct ModeCommand {
    #[arg(short = 'l', long = "loop", action = clap::ArgAction::SetTrue, help = "Set playmode to Loop")]
    loop_mode: bool,
    #[arg(short = 's', long = "shuffle", action = clap::ArgAction::SetTrue, help = "Set playmode to Shuffle")]
    shuffle_mode: bool,
    #[arg(short = 'r', long = "repeat", action = clap::ArgAction::SetTrue, help = "Set playmode to Repeat")]
    repeat_mode: bool,
}

#[derive(Parser)]
struct ImportCommand {
    #[arg(short = 'f', long = "fid", help = "The favorite ID to import")]
    fid: Option<String>,
    #[arg(short = 'b', long = "bvid", help = "The bvid to import")]
    bvid: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Track {
    bvid: String,
    cid: String,
    title: String,
    owner: String,
}

#[derive(Serialize, Deserialize)]
struct Favorite {
    tracks: Vec<Track>,
}

#[tokio::main]
async fn main() -> StdResult<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let connection = Connection::session().await?;
    let proxy = MyPlayerProxy::new(&connection).await?;

    match cli.command {
        Commands::Play => {
            proxy.play().await?;
            println!("继续播放");
        }
        Commands::Pause => {
            proxy.pause().await?;
            println!("暂停播放");
        }
        Commands::Next => {
            proxy.next().await?;
            println!("播放下一首");
        }
        Commands::Previous => {
            proxy.previous().await?;
            println!("播放上一首");
        }
        Commands::Stop => {
            proxy.stop().await?;
            println!("rosesong已退出");
        }
        Commands::Mode(mode_cmd) => {
            if mode_cmd.loop_mode {
                proxy.set_mode("Loop").await?;
                println!("设置为循环播放");
            } else if mode_cmd.shuffle_mode {
                proxy.set_mode("Shuffle").await?;
                println!("设置为随机播放");
            } else if mode_cmd.repeat_mode {
                proxy.set_mode("Repeat").await?;
                println!("设置为单曲循环");
            } else {
                eprintln!("没有这个播放模式");
            }
        }
        Commands::Import(import_cmd) => {
            if let Err(e) = import_favorite(import_cmd.fid, import_cmd.bvid).await {
                eprintln!("导入出现错误: {}", e);
            }
        }
    }

    Ok(())
}

async fn initialize_directories() -> StdResult<String, ApplicationError> {
    let home_dir = std::env::var("HOME")?;

    // Define the required directories
    let required_dirs = [
        format!("{}/.config/rosesong/playlists", home_dir),
        format!("{}/.config/rosesong/settings", home_dir),
    ];

    // Ensure all directories exist
    for dir in &required_dirs {
        fs::create_dir_all(dir)?;
    }

    // Check if playlist.toml exists, if not, create an empty one
    let playlist_path = format!("{}/.config/rosesong/playlists/playlist.toml", home_dir);
    if !Path::new(&playlist_path).exists() {
        fs::write(&playlist_path, "")?;
    }

    Ok(format!("{}/.config/rosesong/playlists", home_dir))
}

async fn import_favorite(
    fid: Option<String>,
    bvid: Option<String>,
) -> StdResult<(), ApplicationError> {
    let client = reqwest::Client::new();

    // Initialize directories and get the playlist directory
    let playlist_path = initialize_directories().await?.clone() + "/playlist.toml";

    println!("正在获取相关信息");

    let video_data_list = get_video_data(&client, fid.as_deref(), bvid.as_deref()).await?;

    let mut new_tracks = Vec::new();
    for video_data in video_data_list {
        new_tracks.push(Track {
            bvid: video_data.bvid.clone(),
            cid: video_data.cid.to_string().clone(),
            title: video_data.title.clone(),
            owner: video_data.owner.name.clone(),
        });
    }

    // Read existing content from playlist.toml if it exists
    let mut existing_tracks = if Path::new(&playlist_path).exists() {
        let content = fs::read_to_string(&playlist_path).map_err(ApplicationError::Io)?;
        toml::from_str::<Favorite>(&content)
            .map(|favorite| favorite.tracks)
            .unwrap_or_else(|_| Vec::new())
    } else {
        Vec::new()
    };

    // Create a set of existing bvids for easy lookup
    let existing_bvids: HashSet<_> = existing_tracks
        .iter()
        .map(|track| track.bvid.clone())
        .collect();

    // Update existing tracks with new tracks
    for track in existing_tracks.iter_mut() {
        if let Some(new_track) = new_tracks.iter().find(|t| t.bvid == track.bvid) {
            *track = new_track.clone();
        }
    }

    // Append new tracks that do not exist in the existing tracks
    for new_track in new_tracks {
        if !existing_bvids.contains(&new_track.bvid) {
            existing_tracks.push(new_track);
        }
    }

    let favorite = Favorite {
        tracks: existing_tracks,
    };

    // Serialize to TOML and write to playlist.toml
    let toml_content = toml::to_string(&favorite).map_err(|_| {
        ApplicationError::DataParsingError("Failed to serialize tracks to TOML".to_string())
    })?;
    let mut file = fs::File::create(&playlist_path).map_err(ApplicationError::Io)?;
    file.write_all(toml_content.as_bytes())
        .map_err(ApplicationError::Io)?;

    println!("导入成功");

    Ok(())
}
