// src/messages.rs
pub enum PlayerMessage {
    Play,
    Pause,
    Next,
    Previous,
    SetPosition(u64),
}
