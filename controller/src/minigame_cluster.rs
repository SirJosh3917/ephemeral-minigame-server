use crate::brain::{BrainMsg, ConnectionInfo};
use crate::transport::{Kind, Packet, WriteChannel, WriteChannelError};
use derive_more::Display;
use log::{info, trace, warn};
use std::collections::VecDeque;
use std::future::Future;
use std::time::Duration;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;

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

pub struct MinigameClusterHandle {
    pub write: UnboundedSender<ClusterMsg>,
}

impl MinigameClusterHandle {
    pub fn start(kind: String, to_brain: UnboundedSender<BrainMsg>) -> Self {
        let (write, read) = unbounded_channel();
        tokio::task::spawn(run_minigame_cluster(kind, to_brain, write.clone(), read));

        MinigameClusterHandle { write }
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
}

#[derive(Debug)]
pub enum ClusterMsg {
    PushServer(MinigameServer),
    PopServer(ConnectionInfo),
    QueueServer(oneshot::Sender<ServerName>),
    TimerCompleted(i32),
    UpdateActive { name: ServerName, active: bool },
    ServerPong(i32, ServerName),
}

#[derive(Debug)]
pub struct MinigameServer {
    pub name: String,
    pub active: bool,
    pub writer: WriteChannel,
}

impl MinigameServer {
    pub async fn ping(&mut self, timer: i32) -> Result<(), WriteChannelError> {
        self.writer.write_next(&Packet::Ping { timer }).await
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Display)]
pub struct ServerName(#[display(fmt = "{}")] pub String);

async fn run_minigame_cluster(
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
                let ClusterQueueState::RecvPong(server) = state else {
					trace!("cluster {kind}: late ServerPong detected");
					continue;
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
                let ClusterQueueState::RecvPong(server) = state else {
					warn!("cluster {kind}: got TimerCompleted message, but in state {state}");
					continue;
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
