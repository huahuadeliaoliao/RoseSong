use clap::{Parser, ValueEnum};
use zbus::zvariant::OwnedValue;
use zbus::Connection;

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

async fn send_command(command: &str) -> Result<String, Box<dyn std::error::Error>> {
    let connection = Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &connection,
        "com.rosesong.Player",
        "/com/rosesong/Player",
        "com.rosesong.Player",
    )
    .await?;

    let result: Result<OwnedValue, _> = match command {
        "play" => proxy.call("play", &()).await,
        "pause" => proxy.call("pause", &()).await,
        "next" => proxy.call("next", &()).await,
        "previous" => proxy.call("previous", &()).await,
        "stop" => proxy.call("stop", &()).await,
        _ => return Ok("Unknown command".to_string()),
    };

    match result {
        Ok(_) => Ok(format!("Command '{}' processed", command)),
        Err(e) => Ok(format!("Failed to send command: {}", e)),
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let command = match cli.command {
        Commands::Play => "play",
        Commands::Pause => "pause",
        Commands::Next => "next",
        Commands::Previous => "previous",
        Commands::Stop => "stop",
    };

    match send_command(command).await {
        Ok(message) => println!("{}", message),
        Err(e) => eprintln!("Failed to send command: {}", e),
    }
}
