use clap::{Parser, Subcommand};
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
    async fn set_mode(&self, mode: &str) -> Result<()>;
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
        Commands::Mode(mode_cmd) => {
            if mode_cmd.loop_mode {
                proxy.set_mode("Loop").await?;
                println!("Mode set to Loop");
            } else if mode_cmd.shuffle_mode {
                proxy.set_mode("Shuffle").await?;
                println!("Mode set to Shuffle");
            } else if mode_cmd.repeat_mode {
                proxy.set_mode("Repeat").await?;
                println!("Mode set to Repeat");
            } else {
                eprintln!("No valid mode selected");
            }
        }
    }

    Ok(())
}
