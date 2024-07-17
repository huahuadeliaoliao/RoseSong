use clap::{Parser, ValueEnum};
use zbus::{proxy, Connection, Result};

#[proxy(
    interface = "org.rosesong.Player",
    default_service = "org.rosesong.Player",
    default_path = "/org/rosesong/Player"
)]
trait MyPlayer {
    async fn play(&self) -> Result<()>;
    async fn pause(&self) -> Result<()>;
    async fn next(&self) -> Result<()>;
    async fn previous(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
}

#[derive(Parser)]
#[command(name = "rsg", about = "Control the rosesong player.")]
struct Cli {
    #[arg(value_enum)]
    command: Commands,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Commands {
    Play,
    Pause,
    Next,
    Previous,
    Stop,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let connection = Connection::session().await?;
    let proxy = MyPlayerProxy::new(&connection).await?;

    match cli.command {
        Commands::Play => {
            proxy.play().await?;
            println!("Play command sent");
        }
        Commands::Pause => {
            proxy.pause().await?;
            println!("Pause command sent");
        }
        Commands::Next => {
            proxy.next().await?;
            println!("Next command sent");
        }
        Commands::Previous => {
            proxy.previous().await?;
            println!("Previous command sent");
        }
        Commands::Stop => {
            proxy.stop().await?;
            println!("Stop command sent");
        }
    }

    Ok(())
}
