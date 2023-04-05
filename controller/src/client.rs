use crate::brain::ConnectionInfo;
use crate::minigame_cluster::ServerName;
use crate::transport::{Kind, Packet, ReadChannel, ReadChannelError, WriteChannel};
use crate::{BrainMsg, ClusterMsg};

use log::{error, info, trace, warn};
use std::net::SocketAddr;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::mpsc::error::SendError;
use tokio::{net::TcpStream, sync::mpsc::UnboundedSender};

pub async fn start_client_listener(sender: UnboundedSender<BrainMsg>) {
    let addr: SocketAddr = ([0, 0, 0, 0], 25550).into();
    let listener = TcpListener::bind(addr).await.unwrap();
    info!("listening on {addr}");

    // listen for new clients
    while let Ok((connection, address)) = listener.accept().await {
        trace!("new connection received: {address}");

        let sender = sender.clone();
        tokio::task::spawn(async move {
            match handle_client(sender, connection, address).await {
                Ok(_) => info!("{address}: disconnected"),
                Err(error) => warn!("{address}: disconnected with error: {error}"),
            };
        });
    }
}

#[derive(Error, Debug)]
pub enum HandleClientError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("ReadChannelError: {0}")]
    ChannelError(#[from] ReadChannelError),
    #[error("Did not receive initial authentication packet, instead received: {0:?}")]
    InitialAuthPacket(Packet),
    #[error("Received authentication packet during normal communication: {0:?}")]
    SpuriousPacket(Packet),
    #[error("Unable to send message to brain")]
    SendBrainError(#[from] SendError<BrainMsg>),
}

pub async fn handle_client(
    to_brain: UnboundedSender<BrainMsg>,
    connection: TcpStream,
    address: SocketAddr,
) -> Result<!, HandleClientError> {
    info!("{address}: client connected");

    let (read, write) = connection.into_split();
    let mut reader = ReadChannel::new(read);
    let writer = WriteChannel::new(write);

    // read authentication packet
    let packet = reader.read_next().await?;
    trace!("{address}: initial packet received: {packet:?}");

    let conn = match packet {
        Packet::Authentication { name, kind, ip } => {
            let stated_address: SocketAddr = ip
                .trim_matches('/')
                .parse()
                .expect("expected valid IP address");

            let mut conn_address = address;
            conn_address.set_port(stated_address.port());

            let conn = ConnectionInfo {
                name,
                kind,
                address: conn_address,
            };

            trace!("{address}: registering connection as {conn:?}");
            to_brain.send(BrainMsg::NewConn {
                writer,
                conn: conn.clone(),
            })?;

            conn
        }
        other => return Err(HandleClientError::InitialAuthPacket(other)),
    };

    info!("{address}: ready, listening for messages");

    // read packets
    let result = try {
        loop {
            let packet = reader.read_next().await?;
            trace!("{address}: sent packet {packet:?}");

            match packet {
                Packet::Request { kind, player } => {
                    to_brain.send(BrainMsg::Dispatch { kind, player })?;
                }
                Packet::UpdateActive { active } if let Kind::Minigame { kind } = &conn.kind => {
                    to_brain.send(BrainMsg::ClusterForward {
                        minigame_kind: kind.clone(),
                        msg: ClusterMsg::UpdateActive { name: ServerName(conn.name.clone()), active }
                    })?;
                }
                Packet::Pong { timer } if let Kind::Minigame { kind } = &conn.kind => {
                    to_brain.send(BrainMsg::ClusterForward {
                        minigame_kind: kind.clone(),
                        msg: ClusterMsg::ServerPong(timer, ServerName(conn.name.clone()))
                    })?;
                }
                p => return Err(HandleClientError::SpuriousPacket(p)),
            };
        }
    };

    warn!("{address}: connection loop failed, {result:?}");

    to_brain.send(BrainMsg::Unlink { conn })?;

    result
}
