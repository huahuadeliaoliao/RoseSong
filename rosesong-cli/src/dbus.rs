use tokio::sync::mpsc;
use zbus::{fdo, interface, ConnectionBuilder};

use crate::player::PlayerCommand;

#[derive(Clone)]
pub struct PlayerDBus {
    tx: mpsc::Sender<PlayerCommand>,
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
        Ok(())
    }
}

pub async fn run_dbus_server(
    command_sender: mpsc::Sender<PlayerCommand>,
) -> Result<(), Box<dyn std::error::Error>> {
    let player_dbus = PlayerDBus { tx: command_sender };

    let _connection = ConnectionBuilder::session()?
        .name("org.rosesong.Player")?
        .serve_at("/org/rosesong/Player", player_dbus)?
        .build()
        .await?;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}
