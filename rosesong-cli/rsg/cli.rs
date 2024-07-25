mod bilibili;
mod error;

use bilibili::fetch_audio_info::get_video_data;
use clap::{Parser, Subcommand};
use error::ApplicationError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use tokio::{fs, io::AsyncBufReadExt, io::AsyncWriteExt, process::Command};
use zbus::{proxy, Connection};

type StdResult<T, E> = std::result::Result<T, E>;

#[proxy(
    interface = "org.rosesong.Player",
    default_service = "org.rosesong.Player",
    default_path = "/org/rosesong/Player"
)]
trait MyPlayer {
    async fn play(&self) -> zbus::Result<()>;
    async fn play_bvid(&self, bvid: &str) -> zbus::Result<()>;
    async fn pause(&self) -> zbus::Result<()>;
    async fn next(&self) -> zbus::Result<()>;
    async fn previous(&self) -> zbus::Result<()>;
    async fn stop(&self) -> zbus::Result<()>;
    async fn set_mode(&self, mode: &str) -> zbus::Result<()>;
    async fn playlist_change(&self) -> zbus::Result<()>;
    async fn test_connection(&self) -> zbus::Result<()>;
    async fn playlist_is_empty(&self) -> zbus::Result<()>;
}

#[derive(Parser)]
#[command(
    name = "rsg",
    about = "Control the rosesong player.",
    version = "1.0.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "播放指定歌曲或继续播放")]
    Play(PlayCommand),

    #[command(about = "暂停播放")]
    Pause,

    #[command(about = "播放下一首歌曲")]
    Next,

    #[command(about = "播放上一首歌曲")]
    Previous,

    #[command(about = "停止 RoseSong")]
    Stop,

    #[command(about = "设置播放模式")]
    Mode(ModeCommand),

    #[command(about = "添加歌曲到播放列表")]
    Add(AddCommand),

    #[command(about = "在播放列表中查找歌曲")]
    Find(FindCommand),

    #[command(about = "从播放列表中删除歌曲")]
    Delete(DeleteCommand),

    #[command(about = "显示播放列表")]
    Playlist,

    #[command(about = "启动 RoseSong")]
    Start,
}

#[derive(Parser)]
struct PlayCommand {
    #[arg(short = 'b', long = "bvid", help = "要播放的 bvid")]
    bvid: Option<String>,
}

#[derive(Parser)]
struct ModeCommand {
    #[arg(short = 'l', long = "loop", action = clap::ArgAction::SetTrue, help = "设置播放模式为循环播放")]
    loop_mode: bool,
    #[arg(short = 's', long = "shuffle", action = clap::ArgAction::SetTrue, help = "设置播放模式为随机播放")]
    shuffle_mode: bool,
    #[arg(short = 'r', long = "repeat", action = clap::ArgAction::SetTrue, help = "设置播放模式为单曲循环")]
    repeat_mode: bool,
}

#[derive(Parser)]
struct AddCommand {
    #[arg(short = 'f', long = "fid", help = "要导入的收藏夹 ID")]
    fid: Option<String>,
    #[arg(short = 'b', long = "bvid", help = "要导入的 bvid")]
    bvid: Option<String>,
}

#[derive(Parser)]
struct FindCommand {
    #[arg(short = 'b', long = "bvid", help = "按 bvid 查找")]
    bvid: Option<String>,
    #[arg(short = 'c', long = "cid", help = "按 cid 查找")]
    cid: Option<String>,
    #[arg(short = 't', long = "title", help = "按标题查找")]
    title: Option<String>,
    #[arg(short = 'o', long = "owner", help = "按作者查找")]
    owner: Option<String>,
}

#[derive(Parser)]
struct DeleteCommand {
    #[arg(short = 'b', long = "bvid", help = "按 bvid 删除")]
    bvid: Option<String>,
    #[arg(short = 'c', long = "cid", help = "按 cid 删除")]
    cid: Option<String>,
    #[arg(short = 'o', long = "owner", help = "按作者删除")]
    owner: Option<String>,
    #[arg(short = 'a', long = "all", help = "删除所有曲目")]
    all: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct Track {
    bvid: String,
    cid: String,
    title: String,
    owner: String,
}

#[derive(Serialize, Deserialize)]
struct Playlist {
    tracks: Vec<Track>,
}

#[tokio::main]
async fn main() -> StdResult<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let connection = Connection::session().await?;
    let proxy = MyPlayerProxy::new(&connection).await?;

    match cli.command {
        Commands::Play(play_cmd) => {
            if let Some(bvid) = play_cmd.bvid {
                if !is_rosesong_running(&proxy).await? {
                    eprintln!("rosesong 没有处于运行状态");
                } else if is_playlist_empty().await? {
                    eprintln!("当前播放列表为空，请先添加歌曲");
                } else {
                    proxy.play_bvid(&bvid).await?;
                    println!("播放指定bvid");
                }
            } else {
                if !is_rosesong_running(&proxy).await? {
                    eprintln!("rosesong 没有处于运行状态");
                } else if is_playlist_empty().await? {
                    eprintln!("当前播放列表为空，请先添加歌曲");
                } else {
                    proxy.play().await?;
                    println!("继续播放");
                }
            }
        }
        Commands::Pause => {
            if !is_rosesong_running(&proxy).await? {
                eprintln!("rosesong 没有处于运行状态");
            } else if is_playlist_empty().await? {
                eprintln!("当前播放列表为空，请先添加歌曲");
            } else {
                proxy.pause().await?;
                println!("暂停播放");
            }
        }
        Commands::Next => {
            if !is_rosesong_running(&proxy).await? {
                eprintln!("rosesong 没有处于运行状态");
            } else if is_playlist_empty().await? {
                eprintln!("当前播放列表为空，请先添加歌曲");
            } else {
                proxy.next().await?;
                println!("播放下一首");
            }
        }
        Commands::Previous => {
            if !is_rosesong_running(&proxy).await? {
                eprintln!("rosesong 没有处于运行状态");
            } else if is_playlist_empty().await? {
                eprintln!("当前播放列表为空，请先添加歌曲");
            } else {
                proxy.previous().await?;
                println!("播放上一首");
            }
        }
        Commands::Stop => {
            if is_rosesong_running(&proxy).await? {
                proxy.stop().await?;
                println!("rosesong已退出");
            } else {
                eprintln!("rosesong 没有处于运行状态");
            }
        }
        Commands::Mode(mode_cmd) => {
            if !is_rosesong_running(&proxy).await? {
                eprintln!("rosesong 没有处于运行状态");
            } else if is_playlist_empty().await? {
                eprintln!("当前播放列表为空，请先添加歌曲");
            } else {
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
        }
        Commands::Add(add_cmd) => {
            if let Err(e) = add_tracks(add_cmd.fid, add_cmd.bvid, &proxy).await {
                eprintln!("导入出现错误: {}", e);
            }
        }
        Commands::Delete(delete_cmd) => {
            if let Err(e) = delete_tracks(
                delete_cmd.bvid,
                delete_cmd.cid,
                delete_cmd.owner,
                delete_cmd.all,
                &proxy,
            )
            .await
            {
                eprintln!("删除出现错误: {}", e);
            }
        }
        Commands::Find(find_cmd) => {
            if !is_rosesong_running(&proxy).await? {
                eprintln!("rosesong 没有处于运行状态");
            } else if is_playlist_empty().await? {
                eprintln!("当前播放列表为空，请先添加歌曲");
            } else {
                if let Err(e) =
                    find_track(find_cmd.bvid, find_cmd.cid, find_cmd.title, find_cmd.owner).await
                {
                    eprintln!("查找出现错误: {}", e);
                }
            }
        }
        Commands::Playlist => {
            if is_playlist_empty().await? {
                eprintln!("当前播放列表为空，请先添加歌曲");
            } else if let Err(e) = display_playlist().await {
                eprintln!("显示播放列表出现错误: {}", e);
            }
        }
        Commands::Start => {
            start_rosesong(&proxy).await?;
        }
    }

    Ok(())
}

async fn is_rosesong_running(
    proxy: &MyPlayerProxy<'_>,
) -> StdResult<bool, Box<dyn std::error::Error>> {
    match proxy.test_connection().await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

async fn is_playlist_empty() -> StdResult<bool, ApplicationError> {
    let playlist_path = initialize_directories().await?.clone() + "/playlist.toml";

    if !Path::new(&playlist_path).exists() {
        return Ok(true);
    }

    let content = fs::read_to_string(&playlist_path)
        .await
        .map_err(ApplicationError::Io)?;
    Ok(content.trim().is_empty())
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
        fs::create_dir_all(dir).await?;
    }

    // Check if playlist.toml exists, if not, create an empty one
    let playlist_path = format!("{}/.config/rosesong/playlists/playlist.toml", home_dir);
    if !Path::new(&playlist_path).exists() {
        fs::write(&playlist_path, "").await?;
    }

    Ok(format!("{}/.config/rosesong/playlists", home_dir))
}

async fn start_rosesong(proxy: &MyPlayerProxy<'_>) -> StdResult<(), Box<dyn std::error::Error>> {
    if is_rosesong_running(proxy).await? {
        println!("RoseSong 当前已经处于运行状态");
        return Ok(());
    }

    let exe_dir = std::env::current_exe()?
        .parent()
        .ok_or("Failed to get the directory of the executable")?
        .to_path_buf();
    let rosesong_path = exe_dir.join("rosesong");

    if !rosesong_path.exists() {
        return Err("rosesong executable not found in the same directory".into());
    }

    let child = Command::new(rosesong_path)
        .spawn()
        .expect("Failed to start rosesong");

    println!("RoseSong 成功启动，进程 ID: {:?}", child.id());

    Ok(())
}

async fn add_tracks(
    fid: Option<String>,
    bvid: Option<String>,
    proxy: &MyPlayerProxy<'_>,
) -> StdResult<(), ApplicationError> {
    let playlist_path = initialize_directories().await?.clone() + "/playlist.toml";
    let old_content = fs::read_to_string(&playlist_path).await.unwrap_or_default();

    // Add tracks logic
    import_favorite_or_bvid(fid, bvid).await?;

    let new_content = fs::read_to_string(&playlist_path).await.unwrap_or_default();
    if old_content != new_content {
        proxy.playlist_change().await?;
    }

    Ok(())
}

async fn import_favorite_or_bvid(
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
        let content = fs::read_to_string(&playlist_path)
            .await
            .map_err(ApplicationError::Io)?;
        toml::from_str::<Playlist>(&content)
            .map(|playlist| playlist.tracks)
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

    let playlist = Playlist {
        tracks: existing_tracks,
    };

    // Serialize to TOML and write to playlist.toml
    let toml_content = toml::to_string(&playlist).map_err(|_| {
        ApplicationError::DataParsingError("Failed to serialize tracks to TOML".to_string())
    })?;
    let mut file = fs::File::create(&playlist_path)
        .await
        .map_err(ApplicationError::Io)?;
    file.write_all(toml_content.as_bytes())
        .await
        .map_err(ApplicationError::Io)?;

    println!("导入成功");

    Ok(())
}

async fn delete_tracks(
    bvid: Option<String>,
    cid: Option<String>,
    owner: Option<String>,
    all: bool,
    proxy: &MyPlayerProxy<'_>,
) -> StdResult<(), ApplicationError> {
    let playlist_path = initialize_directories().await?.clone() + "/playlist.toml";
    let old_content = fs::read_to_string(&playlist_path).await.unwrap_or_default();

    // Delete tracks logic
    delete_track(bvid, cid, owner, all).await?;

    let new_content = fs::read_to_string(&playlist_path).await.unwrap_or_default();
    if old_content != new_content {
        if is_playlist_empty().await? {
            proxy.playlist_is_empty().await?;
        } else {
            proxy.playlist_change().await?;
        }
    }

    Ok(())
}

async fn delete_track(
    bvid: Option<String>,
    cid: Option<String>,
    owner: Option<String>,
    all: bool,
) -> StdResult<(), ApplicationError> {
    let playlist_path = initialize_directories().await?.clone() + "/playlist.toml";

    if !Path::new(&playlist_path).exists() {
        eprintln!("播放列表文件不存在");
        return Ok(());
    }

    if all {
        println!("即将清空播放列表，是否确认删除所有歌曲？(y/n)");
        let mut confirmation = String::new();
        let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
        stdin
            .read_line(&mut confirmation)
            .await
            .expect("Failed to read line");

        if confirmation.trim().eq_ignore_ascii_case("y") {
            fs::write(&playlist_path, "")
                .await
                .map_err(ApplicationError::Io)?;
            println!("播放列表已清空");
        } else {
            println!("取消清空操作");
        }

        return Ok(());
    }

    let content = fs::read_to_string(&playlist_path)
        .await
        .map_err(ApplicationError::Io)?;
    let mut playlist: Playlist = toml::from_str(&content).map_err(|_| {
        ApplicationError::DataParsingError("Failed to parse playlist.toml".to_string())
    })?;

    // Collect tracks to delete
    let mut tracks_to_delete: Vec<Track> = Vec::new();

    if let Some(bvid) = bvid {
        tracks_to_delete.extend(
            playlist
                .tracks
                .iter()
                .filter(|track| track.bvid == bvid)
                .cloned(),
        );
    }
    if let Some(cid) = cid {
        tracks_to_delete.extend(
            playlist
                .tracks
                .iter()
                .filter(|track| track.cid == cid)
                .cloned(),
        );
    }
    if let Some(owner) = owner {
        tracks_to_delete.extend(
            playlist
                .tracks
                .iter()
                .filter(|track| track.owner.contains(&owner))
                .cloned(),
        );
    }

    // If no tracks to delete found, print message and return
    if tracks_to_delete.is_empty() {
        println!("没有找到符合条件的track");
        return Ok(());
    }

    // Print the number of tracks to delete and ask for confirmation
    println!(
        "即将删除 {} 首歌曲，是否确认删除？(y/n)",
        tracks_to_delete.len()
    );
    let mut confirmation = String::new();
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    stdin
        .read_line(&mut confirmation)
        .await
        .expect("Failed to read line");

    if confirmation.trim().eq_ignore_ascii_case("y") {
        // Perform the deletion
        playlist
            .tracks
            .retain(|track| !tracks_to_delete.contains(track));

        // Serialize to TOML and write to playlist.toml
        let toml_content = toml::to_string(&playlist).map_err(|_| {
            ApplicationError::DataParsingError("Failed to serialize tracks to TOML".to_string())
        })?;
        let mut file = fs::File::create(&playlist_path)
            .await
            .map_err(ApplicationError::Io)?;
        file.write_all(toml_content.as_bytes())
            .await
            .map_err(ApplicationError::Io)?;

        println!("删除成功");
    } else {
        println!("取消删除操作");
    }

    Ok(())
}

async fn find_track(
    bvid: Option<String>,
    cid: Option<String>,
    title: Option<String>,
    owner: Option<String>,
) -> StdResult<(), ApplicationError> {
    let playlist_path = initialize_directories().await?.clone() + "/playlist.toml";

    if !Path::new(&playlist_path).exists() {
        eprintln!("播放列表文件不存在");
        return Ok(());
    }

    let content = fs::read_to_string(&playlist_path)
        .await
        .map_err(ApplicationError::Io)?;
    let playlist: Playlist = toml::from_str(&content).map_err(|_| {
        ApplicationError::DataParsingError("Failed to parse playlist.toml".to_string())
    })?;

    let mut results = playlist.tracks.clone();

    if let Some(bvid) = bvid {
        results = results
            .into_iter()
            .filter(|track| track.bvid == bvid)
            .collect();
    }
    if let Some(cid) = cid {
        results = results
            .into_iter()
            .filter(|track| track.cid == cid)
            .collect();
    }
    if let Some(title) = title {
        results = results
            .into_iter()
            .filter(|track| track.title.contains(&title))
            .collect();
    }
    if let Some(owner) = owner {
        results = results
            .into_iter()
            .filter(|track| track.owner.contains(&owner))
            .collect();
    }

    if results.is_empty() {
        println!("没有找到符合条件的track");
    } else {
        for track in results {
            println!(
                "bvid: {}, cid: {}, title: {}, owner: {}",
                track.bvid, track.cid, track.title, track.owner
            );
        }
    }

    Ok(())
}

async fn display_playlist() -> StdResult<(), ApplicationError> {
    let playlist_path = initialize_directories().await?.clone() + "/playlist.toml";

    if !Path::new(&playlist_path).exists() {
        eprintln!("播放列表文件不存在");
        return Ok(());
    }

    let content = fs::read_to_string(&playlist_path)
        .await
        .map_err(ApplicationError::Io)?;
    let playlist: Playlist = toml::from_str(&content).map_err(|_| {
        ApplicationError::DataParsingError("Failed to parse playlist.toml".to_string())
    })?;

    let tracks = playlist.tracks;
    let total_tracks = tracks.len();
    let page_size = 10;
    let total_pages = (total_tracks + page_size - 1) / page_size;

    let mut current_page = 1;

    loop {
        let start = (current_page - 1) * page_size;
        let end = (start + page_size).min(total_tracks);

        println!("第 {} 页，共 {} 页", current_page, total_pages);
        for (i, track) in tracks[start..end].iter().enumerate() {
            println!(
                "{}. bvid: {}, cid: {}, title: {}, owner: {}",
                start + i + 1,
                track.bvid,
                track.cid,
                track.title,
                track.owner
            );
        }

        println!("\n请输入页码（1-{}），或输入 'q' 退出：", total_pages);

        let mut input = String::new();
        let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
        stdin
            .read_line(&mut input)
            .await
            .expect("Failed to read line");

        if input.trim().eq_ignore_ascii_case("q") {
            break;
        }

        match input.trim().parse::<usize>() {
            Ok(page) if page >= 1 && page <= total_pages => current_page = page,
            _ => println!("无效的输入，请输入有效的页码或 'q' 退出"),
        }
    }

    Ok(())
}
