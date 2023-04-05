#![feature(try_blocks)]
#![feature(once_cell)]
#![feature(never_type)]
#![feature(if_let_guard)]

use bollard::container::Config;
use bollard::network::ConnectNetworkOptions;
use bollard::service::EndpointSettings;
use bollard::Docker;
use derive_more::Display;
use log::{error, info, trace, warn};
use rmp_serde::{Deserializer, Serializer};
use rouille::Response;
use serde::{Deserialize, Serialize};
// use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::future::Future;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot;
use tokio::{
    io::{AsyncReadExt, BufWriter},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

#[derive(Clone, Copy)]
pub enum ComputerStatus {
    Starting,
    Online,
    Offline,
}

#[derive(Clone, Default)]
pub struct GlobalComputerMap {
    // BTreeMap for stable order
    data: Arc<Mutex<BTreeMap<String, ComputerStatus>>>,
}

impl GlobalComputerMap {
    pub fn set_status<S: ToString>(&self, computer_name: S, status: ComputerStatus) {
        let mut data = match self.data.lock() {
            Ok(guard) => guard,
            Err(err) => err.into_inner(),
        };

        match status {
            ComputerStatus::Offline => {
                let computer_name = computer_name.to_string();
                data.remove(&computer_name)
            }
            status => data.insert(computer_name.to_string(), status),
        };
    }

    pub fn list_statuses(&self) -> Vec<(String, ComputerStatus)> {
        let data_guard = match self.data.lock() {
            Ok(guard) => guard,
            Err(err) => err.into_inner(),
        };
        let data = data_guard.clone();
        drop(data_guard);

        data.into_iter().collect()
    }
}

fn main() {
    println!("sync starting...");

    env_logger::builder()
        .filter(None, log::LevelFilter::Trace)
        .init();

    let body = async_main();
    return tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the Runtime")
        .block_on(body);
}

async fn async_main() {
    println!("async starting...");

    let computers = GlobalComputerMap::default();

    tokio::task::spawn(serve_web(computers.clone()));

    let addr: SocketAddr = ([0, 0, 0, 0], 25550).into();
    let listener = TcpListener::bind(addr).await.unwrap();
    info!("listening on {addr}");

    // start the brain
    let (sender, receiver) = unbounded_channel();
    let child_sender = sender.clone();
    let child_computers = computers.clone();
    tokio::task::spawn(async move {
        child_computers.set_status("brain", ComputerStatus::Online);
        match brain(child_computers.clone(), child_sender, receiver).await {
            Ok(_) => info!("brain exited successfully!"),
            Err(err) => error!("brain exited unexpectedly: {err:?}"),
        };
        child_computers.set_status("brain", ComputerStatus::Offline);
    });

    sender
        .send(BrainMsg::Spawn { kind: Kind::Lobby })
        .expect("expected to instruct brain to spawn lobby");

    // listen for new clients
    while let Ok((connection, address)) = listener.accept().await {
        trace!("new connection received: {address}");

        let sender = sender.clone();
        tokio::task::spawn(async move {
            let result = handle_client(sender, connection, address).await;

            match result {
                Ok(_) => {
                    info!("{address}: disconnected");
                }
                Err(error) => {
                    warn!("{address}: disconnected with error: {error}");
                }
            };
        });
    }

    info!("shutting down");
}

async fn serve_web(computers: GlobalComputerMap) {
    info!("web server starting on :25580");

    rouille::start_server("0.0.0.0:25580", move |_| {
        let mut response = String::with_capacity(1024);

        for (computer, status) in computers.list_statuses() {
            response.push_str(&computer);
            response.push(',');
            response.push_str(match status {
                ComputerStatus::Starting => "starting",
                ComputerStatus::Online => "online",
                // Could be made more type safe but w/e.
                ComputerStatus::Offline => unreachable!("list_statuses will never return Offline"),
            });
            response.push('\n');
        }

        Response::text(response)
    });
}

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

#[derive(Error, Debug)]
enum HandleClientError {
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

async fn handle_client(
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

#[derive(Debug)]
enum BrainMsg {
    NewConn {
        conn: ConnectionInfo,
        writer: WriteChannel,
    },
    Unlink {
        conn: ConnectionInfo,
    },
    Dispatch {
        kind: Kind,
        player: Option<String>,
    },
    ClusterForward {
        minigame_kind: String,
        msg: ClusterMsg,
    },
    Spawn {
        kind: Kind,
    },
    Transport {
        player: String,
        server: ServerName,
    },
}

#[derive(Debug, Error)]
enum BrainError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Send error (channel closed too early?)")]
    Send(#[from] SendError<ClusterMsg>),
    #[error("Docker error")]
    Docker(#[from] bollard::errors::Error),
}

async fn brain(
    computers: GlobalComputerMap,
    sender: UnboundedSender<BrainMsg>,
    mut receiver: UnboundedReceiver<BrainMsg>,
) -> Result<(), BrainError> {
    info!("brain thread started");

    let docker = Docker::connect_with_unix_defaults()?;

    let mut proxy_server = None;
    let mut lobby_server = None;
    let mut limbo_server = None;
    let mut minigame_servers = HashMap::new();

    // TODO: turn `used_names` into its own struct
    let mut used_names = HashSet::new();

    while let Some(msg) = receiver.recv().await {
        trace!("brain: handling {msg:?}");

        match msg {
            BrainMsg::NewConn {
                mut writer,
                conn:
                    ConnectionInfo {
                        address,
                        name,
                        kind,
                    },
            } => {
                if !used_names.insert(name.clone()) {
                    warn!("brain: name {name} already exists in used_names {used_names:?}");
                }

                computers.set_status(&name, ComputerStatus::Online);

                match kind {
                    Kind::Proxy => {
                        if proxy_server.is_some() {
                            warn!("two proxy servers connected. is something wrong?");
                            writer.writer.shutdown().await?;
                            continue;
                        }

                        proxy_server = Some(writer);
                    }
                    kind @ (Kind::Limbo | Kind::Lobby) => {
                        let proxy_server = match &mut proxy_server {
                            Some(x) => x,
                            None => {
                                trace!("proxy_server not connected, requeueing limbo-connection message");
                                sender
                                    .send(BrainMsg::NewConn {
                                        writer,
                                        conn: ConnectionInfo {
                                            name,
                                            kind,
                                            address,
                                        },
                                    })
                                    .expect("expected to send message to brain");
                                continue;
                            }
                        };

                        match kind {
                            Kind::Limbo => limbo_server = Some(writer),
                            Kind::Lobby => lobby_server = Some(writer),
                            _ => unreachable!(),
                        };

                        let result = proxy_server
                            .write_next(&Packet::LinkServer {
                                name,
                                address: address.ip().to_string(),
                                port: address.port(),
                                priority: kind.priority(),
                            })
                            .await;

                        match result {
                            Ok(_) => {}
                            Err(err) => {
                                warn!("couldn't send LinkServer packet to proxy: {err}")
                            }
                        };
                    }
                    msg_kind @ Kind::Minigame { .. } => {
                        let priority = msg_kind.priority();
                        let kind = match msg_kind {
                            Kind::Minigame { kind } => kind,
                            _ => unreachable!(),
                        };

                        let server = MinigameServer {
                            writer,
                            name: name.clone(),
                            active: true,
                        };

                        (minigame_servers.entry(kind.clone()))
                            .or_insert_with(|| MinigameCluster::start(kind.clone(), sender.clone()))
                            .push_server(server)?;

                        let proxy_server = match &mut proxy_server {
                            Some(x) => x,
                            None => {
                                trace!("proxy_server not connected, requeueing limbo-connection message");
                                todo!("requeue minigame server connecting");
                            }
                        };

                        let result = proxy_server
                            .write_next(&Packet::LinkServer {
                                name,
                                address: address.ip().to_string(),
                                port: address.port(),
                                priority,
                            })
                            .await;

                        match result {
                            Ok(_) => {}
                            Err(err) => {
                                warn!("couldn't send LinkServer packet to proxy: {err}")
                            }
                        };
                    }
                }
            }
            BrainMsg::Unlink { conn } => {
                let conn2 = conn.clone();
                let ConnectionInfo {
                    name,
                    address,
                    kind,
                } = conn;

                computers.set_status(&name, ComputerStatus::Offline);

                // TODO: map names to connections or something?
                if let Kind::Proxy = kind {
                    continue;
                };

                let proxy_server = match &mut proxy_server {
                    Some(x) => x,
                    None => {
                        trace!("proxy_server not connected, requeueing limbo-connection message");
                        todo!("todo: requeue");
                    }
                };

                if !used_names.remove(&name) {
                    warn!("brain: tried to remove name {name} from used_names but it never existed? {used_names:?}");
                }

                let result = proxy_server
                    .write_next(&Packet::UnlinkServer { name })
                    .await;

                match result {
                    Ok(_) => {}
                    Err(err) => {
                        warn!("couldn't send LinkServer packet to proxy: {err}")
                    }
                };

                if let Kind::Minigame { kind } = kind {
                    if let Some(cluster) = minigame_servers.get_mut(&kind.clone()) {
                        cluster.pop_server(conn2)?;
                    } else {
                        warn!("when removing minigame ({conn2:?}), a minigame cluster for this kind ({kind}) did not exist.");
                    }
                }
            }
            BrainMsg::Dispatch { kind, player } => {
                match kind {
                    Kind::Limbo | Kind::Proxy => {
                        warn!("request to spawn {kind:?} denied");
                        continue;
                    }
                    Kind::Lobby => todo!("dispatch to lobby"),
                    Kind::Minigame { kind } => {
                        trace!("brain: initiating queue request of minigame {kind}");
                        let server_name = (minigame_servers.entry(kind.clone()))
                            .or_insert_with(|| MinigameCluster::start(kind.clone(), sender.clone()))
                            .queue_server()?;

                        let sender = sender.clone();
                        tokio::task::spawn(async move {
                            trace!("brain dispatch task ({kind}, {player:?}): waiting for server_name...");
                            let server_name = server_name.await;

                            trace!("brain dispatch task ({kind}, {player:?}): got server: {server_name}");

                            if let Some(player_name) = player.clone() {
                                trace!("brain dispatch task ({kind}, {player:?}): transporting {player_name} to {server_name}");
                                sender
                                    .send(BrainMsg::Transport {
                                        player: player_name,
                                        server: server_name,
                                    })
                                    .unwrap();
                            }
                        });
                    }
                };
            }
            BrainMsg::ClusterForward { minigame_kind, msg } => {
                let cluster = match minigame_servers.get(&minigame_kind) {
                    Some(cluster) => cluster,
                    None => {
                        warn!("brain: unable to forward {msg:?} to cluster {minigame_kind:?} as it does not exist.");
                        continue;
                    }
                };

                cluster.write.send(msg)?;
            }
            BrainMsg::Spawn { kind } => {
                // TODO: maybe use a free list of some kind?
                let mut counter = 0usize;
                let server_name = loop {
                    let name = format!("{kind}-{counter}");

                    if used_names.insert(name.clone()) {
                        break name;
                    }

                    counter += 1;
                };

                computers.set_status(&server_name, ComputerStatus::Starting);

                let mut env = Vec::new();

                env.push("CONTROLLER_IP=controller".to_owned());
                env.push(format!("SERVER_NAME={server_name}"));

                match kind.clone() {
                    Kind::Proxy => unimplemented!("cannot spawn new proxy"),
                    Kind::Limbo => {
                        env.push("SERVER_KIND=Limbo".to_owned());
                    }
                    Kind::Lobby => {
                        env.push("SERVER_KIND=Lobby".to_owned());
                    }
                    Kind::Minigame { kind } => {
                        env.push("SERVER_KIND=Minigame".to_owned());
                        env.push(format!("MINIGAME_KIND={kind}"));
                    }
                };

                let image = match kind {
                    Kind::Lobby => "ems-lobby",
                    Kind::Minigame { .. } => "ems-minigame",
                    _ => unimplemented!(),
                };

                let opts = Config {
                    // hostname: todo!(),
                    // domainname: todo!(),
                    // user: todo!(),
                    // attach_stdin: todo!(),
                    // attach_stdout: todo!(),
                    // attach_stderr: todo!(),
                    // exposed_ports: todo!(),
                    // tty: todo!(),
                    // open_stdin: todo!(),
                    // stdin_once: todo!(),
                    env: Some(env),
                    // cmd: todo!(),
                    // healthcheck: todo!(),
                    // args_escaped: todo!(),
                    image: Some(image.to_owned()),
                    // volumes: todo!(),
                    // working_dir: todo!(),
                    // entrypoint: todo!(),
                    // network_disabled: todo!(),
                    // mac_address: todo!(),
                    // on_build: todo!(),
                    // labels: todo!(),
                    // stop_signal: todo!(),
                    // stop_timeout: todo!(),
                    // shell: todo!(),
                    // host_config: todo!(),
                    ..Default::default()
                };

                trace!("brain: spawning child...");
                let container = docker.create_container::<String, _>(None, opts).await?;

                for warning in container.warnings {
                    trace!("brain: warning {warning}");
                }

                let id = container.id;
                info!("brain: spawned server {id}\n");

                docker
                    .connect_network(
                        "ems_network",
                        ConnectNetworkOptions {
                            container: &id,
                            endpoint_config: EndpointSettings {
                                ..Default::default()
                            },
                        },
                    )
                    .await?;

                info!("brain: connected new server to network!");

                docker.start_container::<String>(&id, None).await?;

                info!("brain: started container!");
            }
            BrainMsg::Transport {
                player,
                server: ServerName(to),
            } => {
                let proxy_server = match &mut proxy_server {
                    Some(x) => x,
                    None => {
                        trace!("proxy_server not connected, requeueing limbo-connection message");
                        todo!("todo: requeue");
                    }
                };

                let result = proxy_server
                    .write_next(&Packet::TransportPlayer { player, to })
                    .await;

                if let Err(err) = result {
                    warn!("brain: unable to send transport packet to proxy: {err}");
                }
            }
        }
    }

    info!("brain thread exiting");
    Ok(())
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

#[derive(Debug, Clone)]
struct ConnectionInfo {
    name: String,
    kind: Kind,
    address: SocketAddr,
}

#[derive(Debug)]
struct MinigameServer {
    name: String,
    active: bool,
    writer: WriteChannel,
}

impl MinigameServer {
    pub async fn ping(&mut self, timer: i32) -> Result<(), WriteChannelError> {
        self.writer.write_next(&Packet::Ping { timer }).await
    }
}

struct MinigameCluster {
    pub write: UnboundedSender<ClusterMsg>,
    // handle: JoinHandle<Result<(), ()>>,
}

#[derive(Display)]
enum ClusterQueueState {
    /// The cluster is currently not attempting to queue players into a minigame server.
    Idle,
    /// When the cluster has sent out to all active servers asking if any of them
    /// is willing to accept players, it is in this state while waiting for players.
    /// It will transition to either [`Idle`] or [`Starting`] after this state.
    ///
    /// [`Idle`]: ClusterQueueState::Idle
    /// [`Starting`]: ClusterQueueState::Starting
    #[display(fmt = "RecvPong")]
    RecvPong(oneshot::Sender<ServerName>),
    /// The cluster has decided to instantiate a new minigame server to queue players
    /// into, and is waiting for the new minigame server to connect. Once the new
    /// minigame server is found, it will transition back into the [`Idle`] state.
    ///
    /// [`Idle`]: ClusterQueueState::Idle
    #[display(fmt = "Starting")]
    Starting(oneshot::Sender<ServerName>),
}

impl MinigameCluster {
    pub fn start(kind: String, to_brain: UnboundedSender<BrainMsg>) -> Self {
        let (write, read) = unbounded_channel();
        let _handle = tokio::task::spawn(MinigameCluster::run(kind, to_brain, write.clone(), read));

        MinigameCluster { write }
    }

    pub fn push_server(&self, server: MinigameServer) -> Result<(), SendError<ClusterMsg>> {
        self.write.send(ClusterMsg::PushServer(server))
    }

    pub fn pop_server(&self, conn: ConnectionInfo) -> Result<(), SendError<ClusterMsg>> {
        self.write.send(ClusterMsg::PopServer(conn))
    }

    pub fn queue_server(&self) -> Result<impl Future<Output = ServerName>, SendError<ClusterMsg>> {
        let (sender, receiver) = oneshot::channel();
        self.write.send(ClusterMsg::QueueServer(sender))?;
        Ok(async move { receiver.await.expect("cannot fail") })
    }

    async fn run(
        kind: String,
        to_brain: UnboundedSender<BrainMsg>,
        writer: UnboundedSender<ClusterMsg>,
        mut reader: UnboundedReceiver<ClusterMsg>,
    ) -> Result<(), ()> {
        info!("cluster {kind}: started");

        // the primary complexity of the minigame cluster is when we need to queue
        // players into a minigame, as there's a lot of message-passing involved
        // with potential for "dead ends" (where messages don't go through).
        //
        // in addition, we can never block the event loop (the receiving loop) as
        // we need to be open to messages (such as new servers or pong replies) to
        // support queueing servers.

        let mut servers = Vec::new();

        let mut state = ClusterQueueState::Idle;

        // the cluster can only fulfill a single queue request at a time, so we buffer
        // queue requests we can't handle
        let mut queue_reqs = VecDeque::new();

        // if no server sends a `Pong` reply, we receive a `TimerCompleted` message,
        // but we need some way to ensure we can ignore TimerCompleted messages if we
        // do receive a Pong reply and the timer thread doesn't know about it.
        let mut timer_now: i32 = 0;

        while let Some(msg) = reader.recv().await {
            trace!("cluster {kind}: received message {msg:?}");

            match msg {
                ClusterMsg::PushServer(server) => {
                    let name = server.name.clone();

                    info!("cluster {kind}: adding server {server:?}. current servers: {servers:?}");
                    servers.push(server);

                    if let ClusterQueueState::Starting(server) = state {
                        state = ClusterQueueState::Idle;
                        server.send(ServerName(name)).expect("expect to respond");

                        // we are now idle: re-queue work if necessary
                        if let Some(respond) = queue_reqs.pop_front() {
                            trace!("cluster {kind}: detected idle state with server queue requests, priming msg loop");
                            writer.send(ClusterMsg::QueueServer(respond)).unwrap();
                        }
                    }
                }
                ClusterMsg::PopServer(server) => {
                    match servers.iter().position(|s| s.name == server.name) {
                        Some(server) => {
                            servers.remove(server);
                            info!("cluster {kind}: removed server {server:?}. current servers: {servers:?}");
                        }
                        None => {
                            warn!("cluster {kind}: unable to find a minigame server for server connection {server:?}. current servers: {servers:?}");
                        }
                    }
                }
                ClusterMsg::UpdateActive {
                    name: ServerName(name),
                    active,
                } => match servers.iter_mut().find(|s| s.name == name) {
                    Some(s) => {
                        s.active = active;
                        info!("cluster {kind}: update server {name}'s active state to: {active}")
                    }
                    None => {
                        warn!("cluster {kind}: unable to find server {name}. current servers: {servers:?}");
                    }
                },
                //
                // from here on, these are messages relating to queueing players
                // into a minigame server.
                //
                ClusterMsg::QueueServer(server) => {
                    // when we receive a `QueueServer` request, we may already be
                    // working on an existing queue request. the `Idle` state is
                    // an invariant that says we can accept requests
                    if !matches!(state, ClusterQueueState::Idle) {
                        queue_reqs.push_back(server);
                        continue;
                    }

                    // switch to the next state to clear the `Idle` invariants as
                    // soon as possible.
                    state = ClusterQueueState::RecvPong(server);

                    // ping all active servers
                    let active_servers = servers.iter_mut().filter(|s| s.active);
                    for server in active_servers {
                        let ping = server.ping(timer_now).await;

                        if let Err(err) = ping {
                            warn!("cluster {kind}: couldn't send ping to {server:?}: {err}");
                        }
                    }

                    // now, we are waiting to receive pings.
                    // if we don't receive any pings, we will be stuck here forever.

                    // start a timer if no servers respond
                    let writer = writer.clone();
                    let kind = kind.clone();
                    let timer = timer_now;
                    tokio::task::spawn(async move {
                        trace!("cluster {kind} timer {timer}: starting now");
                        tokio::time::sleep(Duration::from_secs(1)).await;

                        trace!("cluster {kind} timer {timer}: done");
                        writer
                            .send(ClusterMsg::TimerCompleted(timer))
                            .expect("expected to send cluster msg");
                    });
                }
                ClusterMsg::ServerPong(timer, name) => {
                    if timer != timer_now {
                        trace!("cluster {kind}: late ServerPong detected (timer: {timer}, now: {timer_now})");
                        continue;
                    }

                    // we only want to handle server pongs when we are receiving pongs
                    // TODO: use let else when compiling with rust 1.60
                    // let ClusterQueueState::RecvPong(server) = state else {
                    //     trace!("cluster {kind}: late ServerPong detected");
                    //     continue;
                    // };
                    let server = match state {
                        ClusterQueueState::RecvPong(server) => server,
                        _ => {
                            trace!("cluster {kind}: late ServerPong detected (state mismatch)");
                            continue;
                        }
                    };

                    // send the player to the server
                    server.send(name).expect("expected to respond to query");

                    // switch our state to the next invariant possible: capable of
                    // receiving more queue requests
                    state = ClusterQueueState::Idle;
                    timer_now = timer_now.wrapping_add(1); // ignore older events

                    // we are now idle: re-queue work if necessary
                    if let Some(respond) = queue_reqs.pop_front() {
                        trace!("cluster {kind}: detected idle state with server queue requests, priming msg loop");
                        writer.send(ClusterMsg::QueueServer(respond)).unwrap();
                    }
                }
                ClusterMsg::TimerCompleted(timer) => {
                    // if the `timer_pong` is not the same as `timer`, that means we
                    // have already received a `ClusterMsg::ServerPong` (as ServerPong
                    // will increment `timer`, thus making `timer_pong` =/= `mutex`).
                    if timer_now != timer {
                        trace!("cluster {kind}: late TimerCompleted detected (timer: {timer}, now: {timer_now}");
                        continue;
                    }

                    // if we know the timer message isn't late, let's check that we're
                    // in the correct state
                    // TODO: use let else when compiling with rust 1.60
                    // let ClusterQueueState::RecvPong(server) = state else {
                    //     warn!("cluster {kind}: got TimerCompleted message, but in state {state}");
                    //     continue;
                    // };
                    let server = match state {
                        ClusterQueueState::RecvPong(server) => server,
                        _ => {
                            warn!(
                                "cluster {kind}: got TimerCompleted message, but in state {state}"
                            );
                            continue;
                        }
                    };

                    // if we get a `ServerPong` after this TimerCompleted, we want to
                    // ignore the pong.
                    state = ClusterQueueState::Starting(server);
                    timer_now = timer_now.wrapping_add(1); // ignore older events

                    // tell the brain to start a new server
                    let spawn = BrainMsg::Spawn {
                        kind: Kind::Minigame { kind: kind.clone() },
                    };

                    to_brain.send(spawn).expect("expected to send brain msg");
                }
            }
        }

        info!("minigame cluster {kind} ending");
        Ok(())
    }
}

#[derive(Debug)]
enum ClusterMsg {
    PushServer(MinigameServer),
    PopServer(ConnectionInfo),
    QueueServer(oneshot::Sender<ServerName>),
    TimerCompleted(i32),
    UpdateActive { name: ServerName, active: bool },
    ServerPong(i32, ServerName),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Display)]
struct ServerName(#[display(fmt = "{}")] String);

struct ReadChannel {
    reader: BufReader<OwnedReadHalf>,
}

#[derive(Error, Debug)]
enum ReadChannelError {
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
struct WriteChannel {
    addr: SocketAddr,
    writer: BufWriter<OwnedWriteHalf>,
}

#[derive(Error, Debug)]
enum WriteChannelError {
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
}

#[derive(Error, Debug)]
enum ConnRecError {
    #[error("IO error: {_0}")]
    IoError(#[from] std::io::Error),
}
