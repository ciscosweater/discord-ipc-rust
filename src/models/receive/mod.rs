pub mod commands;
pub mod events;

mod data;

#[derive(Debug)]
pub struct CommandResponse {
    pub nonce: Option<String>,
    pub command: commands::ReturnedCommand,
}

/// Represents values received from the RPC server, either events or command responses
#[derive(Debug)]
pub enum ReceivedItem {
    Event(Box<events::ReturnedEvent>),
    Command(Box<CommandResponse>),
    SocketClosed,
}
