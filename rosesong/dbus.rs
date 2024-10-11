use std::sync::Arc;

use log::info;
use tokio::sync::{mpsc, watch, Mutex};
use zbus::{fdo, interface, ConnectionBuilder};

use crate::player::playlist::PlayMode;
use crate::player::Command;

#[derive(Clone)]
pub struct PlayerDBus {
    tx: mpsc::Sender<Command>,
    stop_signal: watch::Sender<()>,
    playlist_empty: Arc<Mutex<bool>>,
}

#[interface(name = "org.rosesong.Player")]
impl PlayerDBus {
    #[allow(clippy::unused_self)]
    fn test_connection(&self) {}

    async fn play(&self) -> fdo::Result<()> {
        self.tx.send(Command::Play).await.unwrap();
        Ok(())
    }

    async fn play_bvid(&self, bvid: String) -> fdo::Result<()> {
        self.tx.send(Command::PlayBvid(bvid)).await.unwrap();
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.tx.send(Command::Pause).await.unwrap();
        Ok(())
    }

    async fn next(&self) -> fdo::Result<()> {
        self.tx.send(Command::Next).await.unwrap();
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        self.tx.send(Command::Previous).await.unwrap();
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.tx.send(Command::Stop).await.unwrap();
        self.stop_signal.send(()).unwrap();
        Ok(())
    }

    async fn set_mode(&self, mode: String) -> fdo::Result<()> {
        let mode = match mode.as_str() {
            "Loop" => PlayMode::Loop,
            "Shuffle" => PlayMode::Shuffle,
            "Repeat" => PlayMode::Repeat,
            _ => return Err(fdo::Error::Failed("Invalid mode".into())),
        };
        self.tx.send(Command::SetPlayMode(mode)).await.unwrap();
        Ok(())
    }

    async fn playlist_change(&self) -> fdo::Result<()> {
        let mut playlist_empty = self.playlist_empty.lock().await;
        if *playlist_empty {
            *playlist_empty = false;
            self.tx.send(Command::PlaylistIsEmpty).await.unwrap();
        } else {
            self.tx.send(Command::ReloadPlaylist).await.unwrap();
        }
        Ok(())
    }

    async fn playlist_is_empty(&self) -> fdo::Result<()> {
        self.tx.send(Command::Stop).await.unwrap();
        let mut playlist_empty = self.playlist_empty.lock().await;
        *playlist_empty = true;
        Ok(())
    }
}

pub async fn run_dbus_server(
    command_sender: mpsc::Sender<Command>,
    stop_signal: watch::Sender<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    let player_dbus = PlayerDBus {
        tx: command_sender,
        stop_signal: stop_signal.clone(),
        playlist_empty: Arc::new(Mutex::new(false)),
    };

    let _connection = ConnectionBuilder::session()?
        .name("org.rosesong.Player")?
        .serve_at("/org/rosesong/Player", player_dbus)?
        .build()
        .await?;

    let mut stop_receiver = stop_signal.subscribe();

    // Wait for the stop signal
    tokio::select! {
        _ = stop_receiver.changed() => {
            info!("Stop signal received, shutting down DBus server...");
        }
    }

    Ok(())
}
