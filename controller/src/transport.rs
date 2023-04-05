use derive_more::Display;
use log::{error, trace};
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use thiserror::Error;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::{
    io::{AsyncReadExt, BufWriter},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
};

/// A connection between a given server and the controller will **only** communicate
/// in [`Packet`]s. Some packets are not expected to always be able to be sent in
/// specific states, and would be unacceptable to do so.
#[derive(Debug, Serialize, Deserialize)]
pub enum Packet {
    /// The [`Authentication`] packet is sent from the client to the controller to
    /// establish the connection. It contains identifying information about the server,
    /// so that the controller can appropriately handle the messages it may send.
    ///
    /// [`Authentication`]: Packet::Authentication
    Authentication {
        name: String,
        kind: Kind,
        ip: String,
    },
    /// The [`Request`] packet is sent from the client to the controller when the
    /// client wants to make the controller aware of a request that a player wants
    /// to join a specific kind of server. In the event that no player is specified,
    /// this is a request to ensure a server of the specified kind is established.
    ///
    /// [`Request`]: Packet::Request
    Request {
        kind: Kind,
        /// The UUID of the player that wants to connect to the desired server,
        /// if applicable.
        player: Option<String>,
    },
    /// The [`LinkServer`] packet is sent from the controller to the client that
    /// is designated as the proxy server. This is sent to the proxy server upon
    /// a connection so that the proxy server can dynamically add new servers for
    /// players to connect to.
    ///
    /// [`LinkServer`]: Packet::LinkServer
    LinkServer {
        name: String,
        address: String,
        port: u16,
        /// The priority is used to define the order in which linked server takes
        /// precedence over others. The server with the highest priority has all
        /// players forwarded to it on join.
        ///
        /// # Examples
        ///
        /// For every numeric list item, we will state the priority and server name,
        /// and then state the server chosen that the proxy will forward players to.
        ///
        /// 1. `1 - "limbo"`, **Server:** `"limbo"`
        ///
        ///    The only server connected is `"limbo"` with a priority of 1, so all
        ///    players will be forwarded to it.
        ///
        /// 2. `2 - "lobby"`, **Server:** `"lobby"`
        ///
        ///    Now there are two servers: `"limbo"` with priority 1 and `"lobby"`
        ///    with priority 2. Because `"lobby"` has the higher priority, the proxy
        ///    will connect players to it.
        ///
        /// 3. `0 - "minigame-0"`, **Server:** `"lobby"`
        ///
        ///    Because `"lobby"` still has the highest priority (2) than any other
        ///    server (`"limbo"`: 1, `"minigame-0"`: 0), the proxy will still forward
        ///    all players to that server.
        priority: u16,
    },
    /// The [`UnlinkServer`] packet is sent from the controller to the client that
    /// is designated as the proxy server when a connection to the controller is
    /// terminated. If the controller is unable to reach a server, it is safe to
    /// assume that the unreachable server is dead. Therefore, we don't want players
    /// routed to that server in any circumstance, so we inform the proxy to unlink
    /// the connection to that server.
    ///
    /// [`UnlinkServer`]: Packet::UnlinkServer
    UnlinkServer { name: String },
    /// The [`TransportPlayer`] packet is sent from the controller to the client that
    /// is designated as the proxy server when a player is to be transported to
    /// another server. There may be any number of reasons behind the transport, but
    /// the most likely reason is that a [`Request`] packet was able to be fulfilled.
    ///
    /// [`TransportPlayer`]: Packet::TransportPlayer
    /// [`Request`]: Packet::Request
    TransportPlayer { player: String, to: String },
    /// The [`Ping`] packet is sent from the controller (specifically, a minigame
    /// cluster) to a minigame client to ask it if it is accepting players. The
    /// first minigame server to respond with a [`Pong`] packet will have a player
    /// transported to it to participate in the minigame. Servers that do not want
    /// to accept players should not respond to the [`Ping`] packet.
    ///
    /// [`Ping`]: Packet::Ping
    /// [`Pong`]: Packet::Pong
    Ping { timer: i32 },
    /// The [`Pong`] packet is sent from a minigame server to the controller (specifically,
    /// a minigame cluster) only after a [`Ping`] packet has been sent. Sending a
    /// [`Pong`] packet indicates that a server is willing to accept more players,
    /// but servers that do not want players should simply not respond to the [`Ping`]
    /// packet.
    ///
    /// [`Ping`]: Packet::Ping
    /// [`Pong`]: Packet::Pong
    Pong { timer: i32 },
    /// The [`UpdateActive`] packet is sent from a minigame server to the controller
    /// (specifically, a minigame cluster) when the minigame server wants to change
    /// whether or not it's "active". An inactive minigame server will not receive
    /// [`Ping`]s, whereas an active minigame server will receive [`Ping`]s.
    ///
    /// [`Ping`]: Packet::Ping
    /// [`UpdateActive`]: Packet::UpdateActive
    UpdateActive { active: bool },
}

impl Packet {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        Packet::deserialize(&mut Deserializer::new(bytes))
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
        let mut buffer = Vec::new();

        self.serialize(&mut Serializer::new(&mut buffer).with_struct_map())
            .map(|_| buffer)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Display)]
#[serde(tag = "tag", content = "payload")]
pub enum Kind {
    #[display(fmt = "limbo")]
    Limbo,
    #[display(fmt = "proxy")]
    Proxy,
    #[display(fmt = "lobby")]
    Lobby,
    #[display(fmt = "minigame-{kind}")]
    Minigame { kind: String },
}

impl Kind {
    pub fn priority(&self) -> u16 {
        match self {
            Kind::Lobby => 2,
            Kind::Limbo => 1,
            Kind::Minigame { .. } => 0,
            Kind::Proxy => unreachable!(),
        }
    }
}

pub struct ReadChannel {
    reader: BufReader<OwnedReadHalf>,
}

#[derive(Error, Debug)]
pub enum ReadChannelError {
    #[error("IO error: {_0}")]
    IoError(#[from] std::io::Error),
    #[error("Serde error: {_0}")]
    SerdeError(#[from] rmp_serde::decode::Error),
}

impl ReadChannel {
    pub fn new(read_half: OwnedReadHalf) -> Self {
        Self {
            reader: BufReader::new(read_half),
        }
    }

    pub async fn read_next(&mut self) -> Result<Packet, ReadChannelError> {
        let length = self.reader.read_u32().await?;

        let mut buffer = vec![0; length as usize];
        self.reader.read_exact(&mut buffer).await?;

        Ok(Packet::from_bytes(&buffer)?)
    }
}

#[derive(Debug)]
pub struct WriteChannel {
    addr: SocketAddr,
    writer: BufWriter<OwnedWriteHalf>,
}

#[derive(Error, Debug)]
pub enum WriteChannelError {
    #[error("IO error: {_0}")]
    IoError(#[from] std::io::Error),
    #[error("Serde error: {_0}")]
    SerdeError(#[from] rmp_serde::encode::Error),
}

impl WriteChannel {
    pub fn new(write_half: OwnedWriteHalf) -> Self {
        let addr = write_half.peer_addr().expect("expected to get peer_addr");

        Self {
            addr,
            writer: BufWriter::new(write_half),
        }
    }

    pub async fn write_next(&mut self, packet: &Packet) -> Result<(), WriteChannelError> {
        let addr = self.addr;
        trace!("{addr}: sending packet {packet:?}");

        let bytes = packet.to_bytes()?;

        self.writer.write_u32(bytes.len() as u32).await?;
        self.writer.write_all(&bytes).await?;
        self.writer.flush().await?;

        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<(), std::io::Error> {
        self.writer.shutdown().await
    }
}

#[derive(Error, Debug)]
pub enum ConnRecError {
    #[error("IO error: {_0}")]
    IoError(#[from] std::io::Error),
}
