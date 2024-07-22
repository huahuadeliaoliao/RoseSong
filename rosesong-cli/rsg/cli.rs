mod bilibili;
mod error;

use bilibili::fetch_audio_info::{fetch_video_data, get_video_data};
use clap::{Parser, Subcommand};
use error::ApplicationError;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::Path;
use tokio::io::AsyncBufReadExt;
use tokio::sync::oneshot;
use tokio::task;
use zbus::{proxy, Connection};

type StdResult<T, E> = std::result::Result<T, E>; // Alias for std::result::Result

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
    #[arg(short = 'l', long = "loop", action = clap::ArgAction::SetTrue, help = "Set mode to Loop")]
    loop_mode: bool,
    #[arg(short = 's', long = "shuffle", action = clap::ArgAction::SetTrue, help = "Set mode to Shuffle")]
    shuffle_mode: bool,
    #[arg(short = 'r', long = "repeat", action = clap::ArgAction::SetTrue, help = "Set mode to Repeat")]
    repeat_mode: bool,
}

#[derive(Parser)]
struct ImportCommand {
    #[arg(short = 'f', long = "fid", help = "The favorite ID to import")]
    fid: String,
}

#[derive(Serialize)]
struct Track {
    bvid: String,
    cid: String,
    title: String,
    owner: String,
}

#[derive(Serialize)]
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
            if let Err(e) = import_favorite(import_cmd.fid).await {
                eprintln!("Error importing favorite: {}", e);
            }
        }
    }

    Ok(())
}

async fn initialize_directories() -> StdResult<String, ApplicationError> {
    let home_dir = std::env::var("HOME")?;

    // Define the required directories
    let required_dirs = [
        format!("{}/.config/rosesong/favorites", home_dir),
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

    Ok(format!("{}/.config/rosesong/favorites", home_dir))
}

async fn import_favorite(fid: String) -> StdResult<(), ApplicationError> {
    let client = reqwest::Client::new();

    let favorites_dir = initialize_directories().await?;

    let (tx, rx) = oneshot::channel();
    let _input_handle = task::spawn(async move {
        println!("Please enter a name for the favorite:");
        let mut reader = tokio::io::BufReader::new(tokio::io::stdin());
        let mut favorite_name = String::new();
        reader
            .read_line(&mut favorite_name)
            .await
            .map_err(ApplicationError::Io)?;
        let favorite_name = favorite_name.trim().to_string();
        tx.send(favorite_name).map_err(|_| {
            ApplicationError::InvalidInput("Failed to send through oneshot channel".to_string())
        })
    });

    let fetch_handle = task::spawn(async move { get_video_data(&client, Some(&fid), None).await });

    let favorite_name = rx.await.map_err(ApplicationError::OneshotRecvError)?;
    let video_data_list = fetch_handle
        .await
        .map_err(|e| ApplicationError::Io(e.into()))??;

    let mut tracks = Vec::new();
    for video_data in video_data_list {
        tracks.push(Track {
            bvid: video_data.bvid.clone(),
            cid: video_data.cid.to_string().clone(),
            title: video_data.title.clone(),
            owner: video_data.owner.name.clone(),
        });
    }

    let favorite = Favorite { tracks };

    let toml_content = toml::to_string(&favorite).map_err(|_| {
        ApplicationError::DataParsingError("Failed to serialize tracks to TOML".to_string())
    })?;

    let file_path = format!("{}/{}.toml", favorites_dir, favorite_name);
    let mut file = fs::File::create(file_path).map_err(ApplicationError::Io)?;
    file.write_all(toml_content.as_bytes())
        .map_err(ApplicationError::Io)?;

    println!("Favorite imported successfully");

    Ok(())
}
