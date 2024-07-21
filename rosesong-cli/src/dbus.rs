use log::info;
use tokio::sync::{mpsc, watch};
use zbus::{fdo, interface, ConnectionBuilder};

use crate::player::playlist::PlayMode;
use crate::player::PlayerCommand;

#[derive(Clone)]
pub struct PlayerDBus {
    tx: mpsc::Sender<PlayerCommand>,
    stop_signal: watch::Sender<()>,
}

#[interface(name = "org.rosesong.Player")]
impl PlayerDBus {
    async fn play(&self) -> fdo::Result<()> {
        self.tx.send(PlayerCommand::Play).await.unwrap();
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.tx.send(PlayerCommand::Pause).await.unwrap();
        Ok(())
    }

    async fn next(&self) -> fdo::Result<()> {
        self.tx.send(PlayerCommand::Next).await.unwrap();
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        self.tx.send(PlayerCommand::Previous).await.unwrap();
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.tx.send(PlayerCommand::Stop).await.unwrap();
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
        self.tx
            .send(PlayerCommand::SetPlayMode(mode))
            .await
            .unwrap();
        Ok(())
    }
}

pub async fn run_dbus_server(
    command_sender: mpsc::Sender<PlayerCommand>,
    stop_signal: watch::Sender<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    let player_dbus = PlayerDBus {
        tx: command_sender,
        stop_signal: stop_signal.clone(),
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
