#![feature(try_blocks)]
#![feature(once_cell)]
#![feature(never_type)]
#![feature(if_let_guard)]

/// The HTTP module contains everything necessary for the HTTP API of the controller.
/// The HTTP API is used by the Dashboard, to visualize the currently online and starting servers.
pub mod http;
use http::{start_web_server, GlobalComputerMap};

/// The Brain is the brain of the controller. It handles the logic for what to do
/// when new connections connect to it, juggling requests for packets, etc.
pub mod brain;
use brain::{start_brain, BrainMsg};

/// The Transport module contains the low-level primitives for the underlying connection
/// between the controller and servers. It contains primitives to wrap around raw TCP
/// connections, and turns them into exchanges [`transport::Packet`]s
pub mod transport;
use transport::Kind;

/// A minigame cluster is a grouping of minigame servers. These are necessary to
/// facilitate filling in queued players into a running instance, as we must figure
/// out which minigame server is
pub mod minigame_cluster;
use minigame_cluster::ClusterMsg;

/// The client module handles incoming connections as clients. It facilitates
/// basic authentication and talks to the brain.
pub mod client;

use log::info;

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter(None, log::LevelFilter::Trace)
        .init();

    // List of computers and their online/offline statuses for observability into the servers via the dashboard
    let computers = GlobalComputerMap::default();

    start_web_server(computers.clone());

    let sender = start_brain(computers.clone());

    // Spawn a lobby server so that players will join to the server somewhere
    sender
        .send(BrainMsg::Spawn { kind: Kind::Lobby })
        .expect("expected to instruct brain to spawn lobby");

    client::start_client_listener(sender).await;

    info!("shutting down");
}
