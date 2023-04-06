// this is... kinda ugly, but w/e

use crate::http::{ComputerStatus, GlobalComputerMap};
use crate::minigame_cluster::{ClusterMsg, MinigameClusterHandle, MinigameServer, ServerName};
use crate::transport::{Kind, Packet, WriteChannel, WriteChannelError};
use log::{error, info, trace, warn};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub name: String,
    pub kind: Kind,
    pub address: SocketAddr,
}

#[derive(Debug)]
pub enum BrainMsg {
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
pub enum BrainError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Cluster send error (channel closed too early?)")]
    ClusterSend(#[from] SendError<ClusterMsg>),
    #[error("Brain send error (brain channel closed too early?)")]
    BrainSend(#[from] SendError<BrainMsg>),
    #[error("Write channel error (error sending message to connected server)")]
    WriteChannel(#[from] WriteChannelError),
    #[error("Docker error")]
    Docker(#[from] bollard::errors::Error),
}

pub fn start_brain(computers: GlobalComputerMap) -> UnboundedSender<BrainMsg> {
    let (sender, receiver) = unbounded_channel();

    let child_sender = sender.clone();
    tokio::task::spawn(async move {
        computers.set_status("brain", ComputerStatus::Online);

        match start(computers.clone(), child_sender, receiver).await {
            Ok(_) => info!("brain exited successfully!"),
            Err(err) => error!("brain exited unexpectedly: {err:?}"),
        };

        computers.set_status("brain", ComputerStatus::Offline);
    });

    sender
}

pub async fn start(
    computers: GlobalComputerMap,
    sender: UnboundedSender<BrainMsg>,
    mut receiver: UnboundedReceiver<BrainMsg>,
) -> Result<(), BrainError> {
    info!("brain thread started");

    let mut used_names = UniqueNameSet::default();

    // For simplicity, we assume we'll always have an always-online proxy connection.
    //
    // In a more robust solution, we'd likely support multiple proxies which can come
    // in and out of being connected.
    //
    // However, for this simple example, we can assume that if we lose the connection
    // to our proxy (which should be on the same machine), that something so terrible
    // has happened that it's worth it to abandon ship. Let docker health checks deal
    // with it (although, those aren't implemented here).

    info!("waiting for proxy connection...");

    let mut buffer = VecDeque::new();
    let mut proxy_server = None;

    while let Some(msg) = receiver.recv().await {
        println!("received a {msg:?}");

        let BrainMsg::NewConn { conn, writer } = msg else {
            trace!("brain: did not receive connection, queueing into buffer");
            buffer.push_back(msg);
            continue;
        };

        let Kind::Proxy = conn.kind else {
            trace!("brain: did not receive proxy connection, queueing into buffer");
            buffer.push_back(BrainMsg::NewConn { conn, writer });
            continue;
        };

        proxy_server = Some(writer);
        computers.set_status("proxy", ComputerStatus::Online);
        used_names.record("proxy");
        break;
    }

    let Some(mut proxy_server) = proxy_server else {
        unreachable!("brain: unable to find a proxy server, wtf?");
    };

    for queued_msg in buffer {
        sender.send(queued_msg)?;
    }

    // Now that we've established a connection to a proxy server,
    // let's proceed to handle logic for the rest of the server.

    let mut minigame_servers = MacroCluster::new(sender.clone());

    // Used to keep the connection to the lobby server alive
    let mut lobby_server = None;

    let docker = ContainerSpawner::new()?;

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
                // Due to the simplistic guarantee of "one proxy server will always
                // be available" that we established earlier, we don't support multiple
                // proxy servers yet.
                if matches!(kind, Kind::Proxy) {
                    warn!("brain: a second proxy server tried to join, not handling this");
                    writer.shutdown().await?;
                    continue;
                }

                computers.set_status(&name, ComputerStatus::Online);
                if !used_names.record(&name) {
                    warn!("brain: name {name} already exists in used_names {used_names:?}");
                }

                let priority = kind.priority();

                match kind {
                    Kind::Lobby => {
                        // Prevent the writer from getting dropped, and thus the connection stays alive
                        lobby_server = Some(writer);
                    }
                    Kind::Minigame { kind } => {
                        let server = MinigameServer {
                            writer,
                            name: name.clone(),
                            active: true,
                        };

                        minigame_servers.cluster_of(&kind).push_server(server)?;
                    }
                    _ => unreachable!("no other lobby kind supported atmz"),
                };

                proxy_server
                    .write_next(&Packet::LinkServer {
                        name,
                        address: address.ip().to_string(),
                        port: address.port(),
                        priority,
                    })
                    .await?;
            }
            BrainMsg::Unlink { conn } => {
                let conn2 = conn.clone();
                let ConnectionInfo { name, kind, .. } = conn;

                computers.set_status(&name, ComputerStatus::Offline);
                if !used_names.unrecord(&name) {
                    warn!("brain: tried to remove name {name} from used_names but it never existed? {used_names:?}");
                }

                // Due to the simplistic guarantee of "one proxy server will always
                // be available" that we established earlier, we must do this.
                if matches!(kind, Kind::Proxy) {
                    panic!("brain: proxy server died, cannot continue");
                }

                proxy_server
                    .write_next(&Packet::UnlinkServer { name })
                    .await?;

                if let Kind::Minigame { kind } = kind {
                    if let Some(cluster) = minigame_servers.try_get(&kind.clone()) {
                        cluster.pop_server(conn2)?;
                    } else {
                        warn!("when removing minigame ({conn2:?}), a minigame cluster for this kind ({kind}) did not exist.");
                    }
                }
            }
            BrainMsg::Dispatch { kind, player } => {
                match kind {
                    Kind::Limbo | Kind::Proxy => warn!("request to spawn {kind:?} denied"),
                    Kind::Lobby => todo!("dispatch to lobby"),
                    Kind::Minigame { kind } => {
                        let sender = sender.clone();
                        dispatch_to_minigame_server(&mut minigame_servers, kind, sender, player)?;
                    }
                };
            }
            BrainMsg::ClusterForward { minigame_kind, msg } => {
                let Some(cluster) = minigame_servers.try_get(&minigame_kind) else {
                    warn!("brain: unable to forward {msg:?} to cluster {minigame_kind:?} as it does not exist.");
                    continue;
                };

                cluster.write.send(msg)?;
            }
            BrainMsg::Spawn { kind } => {
                let server_name = used_names.next_free_name(&kind);

                computers.set_status(&server_name, ComputerStatus::Starting);

                docker.spawn(server_name, kind).await?;

                info!("brain: started container!");
            }
            BrainMsg::Transport {
                player,
                server: ServerName(to),
            } => {
                proxy_server
                    .write_next(&Packet::TransportPlayer { player, to })
                    .await?;
            }
        }
    }

    info!("brain thread exiting");
    Ok(())
}

fn dispatch_to_minigame_server(
    minigame_servers: &mut MacroCluster,
    kind: String,
    sender: UnboundedSender<BrainMsg>,
    player: Option<String>,
) -> Result<(), BrainError> {
    trace!("brain: initiating queue request of minigame {kind}");

    let server_name = minigame_servers.cluster_of(&kind).queue_server()?;

    tokio::task::spawn(async move {
        trace!("brain dispatch task ({kind}, {player:?}): waiting for server_name...");
        let server_name = server_name.await;
        trace!("brain dispatch task ({kind}, {player:?}): got server: {server_name}");

        let Some(player_name) = player else { return };

        trace!("brain dispatch task ({kind}, {player_name:?}): transporting {player_name} to {server_name}");

        sender
            .send(BrainMsg::Transport {
                player: player_name,
                server: server_name,
            })
            .unwrap();
    });

    Ok(())
}

/// Responsible for managing clusters of [`MinigameClusterHandle`]s
pub struct MacroCluster {
    handles: HashMap<String, MinigameClusterHandle>,
    sender: UnboundedSender<BrainMsg>,
}

impl MacroCluster {
    pub fn new(sender: UnboundedSender<BrainMsg>) -> Self {
        Self {
            handles: HashMap::default(),
            sender,
        }
    }

    pub fn try_get(&mut self, kind: &str) -> Option<&mut MinigameClusterHandle> {
        self.handles.get_mut(kind)
    }

    pub fn cluster_of<S: ToString>(&mut self, kind: S) -> &mut MinigameClusterHandle {
        let kind = kind.to_string();
        let entry = self.handles.entry(kind.clone());
        entry.or_insert_with(|| MinigameClusterHandle::start(kind.to_string(), self.sender.clone()))
    }
}

#[derive(Default, Debug)]
pub struct UniqueNameSet {
    used: HashSet<String>,
}

impl UniqueNameSet {
    pub fn record<S: ToString>(&mut self, name: S) -> bool {
        self.used.insert(name.to_string())
    }

    pub fn unrecord(&mut self, name: &str) -> bool {
        self.used.remove(name)
    }

    pub fn next_free_name<S: std::fmt::Display>(&mut self, basename: S) -> String {
        // TODO: maybe use a free list of some kind?
        let mut counter = 0usize;

        loop {
            let name = format!("{basename}-{counter}");
            counter += 1;

            if self.record(name.clone()) {
                return name;
            }
        }
    }
}

use bollard::container::Config;
use bollard::errors::Error;
use bollard::network::ConnectNetworkOptions;
use bollard::service::EndpointSettings;
use bollard::Docker;

struct ContainerSpawner {
    docker: Docker,
}

impl ContainerSpawner {
    pub fn new() -> Result<Self, Error> {
        let docker = Docker::connect_with_unix_defaults()?;
        Ok(Self { docker })
    }

    pub async fn spawn(&self, server_name: String, kind: Kind) -> Result<(), Error> {
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
            env: Some(env),
            image: Some(image.to_owned()),
            ..Default::default()
        };

        trace!("brain: spawning child...");
        let container = self
            .docker
            .create_container::<String, _>(None, opts)
            .await?;

        for warning in container.warnings {
            trace!("brain: warning {warning}");
        }

        let id = container.id;
        info!("brain: spawned server {id}\n");

        self.docker
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

        self.docker.start_container::<String>(&id, None).await?;

        Ok(())
    }
}
