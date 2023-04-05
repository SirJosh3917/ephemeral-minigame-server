use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use rouille::Response;

/// Starts the web server, which reports the currently known servers and their statuses.
pub fn start_web_server(computers: GlobalComputerMap) {
    tokio::task::spawn(async {
        log::info!("web server starting on :25580");

        rouille::start_server("0.0.0.0:25580", move |_| {
            let mut response = String::with_capacity(1024);

            for (computer, status) in computers.list_statuses() {
                response.push_str(&computer);
                response.push(',');
                response.push_str(match status {
                    ComputerStatus::Starting => "starting",
                    ComputerStatus::Online => "online",
                    // Could be made more type safe but w/e.
                    ComputerStatus::Offline => {
                        unreachable!("list_statuses will never return Offline")
                    }
                });
                response.push('\n');
            }

            Response::text(response)
        });
    });
}

#[derive(Clone, Default)]
pub struct GlobalComputerMap {
    // BTreeMap for stable order
    data: Arc<Mutex<BTreeMap<String, ComputerStatus>>>,
}

#[derive(Clone, Copy)]
pub enum ComputerStatus {
    Starting,
    Online,
    Offline,
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
