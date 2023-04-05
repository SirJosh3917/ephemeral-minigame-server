use crate::http::{ComputerStatus, GlobalComputerMap};
use crate::minigame_cluster::{ClusterMsg, MinigameClusterHandle, MinigameServer, ServerName};
use crate::transport::{Kind, Packet, WriteChannel};
use bollard::container::Config;
use bollard::network::ConnectNetworkOptions;
use bollard::service::EndpointSettings;
use bollard::Docker;
use log::{error, info, trace, warn};
use std::collections::{HashMap, HashSet};
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
    #[error("Send error (channel closed too early?)")]
    Send(#[from] SendError<ClusterMsg>),
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
                            writer.shutdown().await?;
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
                            .or_insert_with(|| {
                                MinigameClusterHandle::start(kind.clone(), sender.clone())
                            })
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
                            .or_insert_with(|| {
                                MinigameClusterHandle::start(kind.clone(), sender.clone())
                            })
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
