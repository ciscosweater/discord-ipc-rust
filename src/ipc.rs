use crate::ipc_socket::DiscordIpcSocket;
use crate::models::receive::commands::{
    GetChannelsData, GetGuildData, GetGuildsData, ReturnedCommand,
};
use crate::models::receive::{CommandResponse, ReceivedItem, events::ReturnedEvent};
use crate::models::send::commands::{AuthenticateArgs, SentCommand};
use crate::models::shared::User;
use crate::utils::create_packet_json_with_nonce;
use crate::{DiscordRPCError, Result};

use serde_json::json;
use tokio::task::JoinHandle;

fn parse_fallback_command(value: &serde_json::Value) -> Option<ReturnedCommand> {
    let cmd = value.get("cmd")?.as_str()?;

    match cmd {
        "GET_GUILDS" => {
            let guilds = serde_json::from_value::<GetGuildsData>(value.get("data")?.clone()).ok()?;
            Some(ReturnedCommand::GetGuilds(guilds))
        }
        "GET_GUILD" => {
            let guild = serde_json::from_value::<GetGuildData>(value.get("data")?.clone()).ok()?;
            Some(ReturnedCommand::GetGuild(guild))
        }
        "GET_CHANNELS" => {
            let channels =
                serde_json::from_value::<GetChannelsData>(value.get("data")?.clone()).ok()?;
            Some(ReturnedCommand::GetChannels(channels.channels))
        }
        _ => None,
    }
}

fn parse_received_item(payload: &str) -> Result<ReceivedItem> {
    let value: serde_json::Value = serde_json::from_str(payload)?;
    let event_name = value.get("evt").and_then(|evt| evt.as_str());
    let command_name = value.get("cmd").and_then(|cmd| cmd.as_str());

    if event_name == Some("ERROR") {
        return Ok(ReceivedItem::Event(Box::new(serde_json::from_value(value)?)));
    }

    if event_name.is_some() && command_name == Some("DISPATCH") {
        return Ok(ReceivedItem::Event(Box::new(serde_json::from_value(value)?)));
    }

    if command_name.is_some() {
        let nonce = value
            .get("nonce")
            .and_then(|nonce| nonce.as_str())
            .map(|nonce| nonce.to_string());
        let command = serde_json::from_value::<ReturnedCommand>(value.clone())
            .or_else(|_| parse_fallback_command(&value).ok_or(DiscordRPCError::CouldNotConnect))?;
        return Ok(ReceivedItem::Command(Box::new(CommandResponse {
            nonce,
            command,
        })));
    }

    if event_name.is_some() {
        return Ok(ReceivedItem::Event(Box::new(serde_json::from_value(value)?)));
    }

    Err(DiscordRPCError::CouldNotConnect)
}

#[allow(dead_code)]
enum OpCodes {
    Handshake,
    Frame,
    Close,
    Ping,
    Pong,
}

pub struct DiscordIpcClient {
    pub client_id: String,
    socket: DiscordIpcSocket,
    event_task: Option<JoinHandle<()>>,
}

impl DiscordIpcClient {
    /// Returns a newly constructed client and the active Discord user
    pub async fn create(client_id: String) -> Result<(DiscordIpcClient, User)> {
        let socket = DiscordIpcSocket::new().await?;
        let mut client = Self {
            client_id,
            socket,
            event_task: None,
        };

        client
            .socket
            .send(
                &json!({ "v": 1, "client_id": client.client_id }).to_string(),
                OpCodes::Handshake as u8,
            )
            .await?;
        let (_opcode, payload) = client.socket.recv().await?;
        let payload = serde_json::from_str(&payload)?;

        match payload {
            ReturnedEvent::Ready(data) => Ok((client, data.user)),
            _ => Err(DiscordRPCError::CouldNotConnect),
        }
    }

    /// Authenticate with the RPC server using an OAuth2 access token
    /// This method will hang if called after setup_event_handler
    pub async fn authenticate(&mut self, access_token: String) -> Result<()> {
        let command = SentCommand::Authenticate(AuthenticateArgs { access_token });
        self.emit_command(&command).await?;
        let (_opcode, payload) = self.socket.recv().await?;
        match parse_received_item(&payload)? {
            ReceivedItem::Command(command) => match command.command {
                ReturnedCommand::Authenticate(_) => Ok(()),
                _ => Err(DiscordRPCError::CouldNotConnect),
            },
            ReceivedItem::Event(event) => match *event {
                ReturnedEvent::Error(error) => Err(DiscordRPCError::Message(format!(
                    "{} ({})",
                    error.message, error.code
                ))),
                _ => Err(DiscordRPCError::CouldNotConnect),
            },
            ReceivedItem::SocketClosed => Err(DiscordRPCError::CouldNotConnect),
        }
    }

    /// Send an arbitrary JSON string payload to the RPC server
    pub async fn emit_string(&mut self, payload: &str) -> Result<()> {
        self.socket.send(payload, OpCodes::Frame as u8).await
    }

    /// Send a command to the RPC server
    pub async fn emit_command(&mut self, command: &SentCommand) -> Result<()> {
        self.emit_command_with_nonce(command).await.map(|_| ())
    }

    /// Send a command to the RPC server and return the nonce used for correlation.
    pub async fn emit_command_with_nonce(&mut self, command: &SentCommand) -> Result<String> {
        let mut command_json = command.to_json()?;
        let (json_string, nonce) = create_packet_json_with_nonce(&mut command_json, None)?;
        log::debug!("IPC sending: {}", json_string);
        self.emit_string(&json_string).await?;
        Ok(nonce)
    }

    /// Set up an event handler that will be called whenever a value is received from the RPC server
    pub async fn setup_event_handler<F>(&mut self, func: F)
    where
        F: Fn(ReceivedItem) + Send + Sync + 'static,
    {
        if let Some(handle) = self.event_task.take() {
            handle.abort();
        }

        let mut socket_clone = self.socket.clone();
        self.event_task = Some(tokio::spawn(async move {
            loop {
                let Ok((_opcode, payload)) = socket_clone.recv().await else {
                    func(ReceivedItem::SocketClosed);
                    break;
                };
                match parse_received_item(&payload) {
                    Ok(item) => func(item),
                    Err(error) => {
                        eprintln!("Failed to deserialize payload {}: {}", payload, error);
                    }
                }
            }
        }));
    }

    /// Remove the event handler
    pub fn remove_event_handler(&mut self) {
        if let Some(handle) = self.event_task.take() {
            handle.abort();
        }
    }
}

impl Drop for DiscordIpcClient {
    fn drop(&mut self) {
        self.remove_event_handler();
    }
}
