// This file is part of TRINCI.
//
// Copyright (C) 2021 Affidaty Spa.
//
// TRINCI is free software: you can redistribute it and/or modify it under
// the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// TRINCI is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License
// for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with TRINCI. If not, see <https://www.gnu.org/licenses/>.

use crate::monitor::worker::{MonitorConfig, MonitorWorker};
use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};
use trinci_core::blockchain::BlockRequestSender;

pub struct MonitorService {
    /// Worker object
    worker: Option<MonitorWorker>,
    /// Worker thread handler
    handler: Option<JoinHandle<MonitorWorker>>,
    /// To check if the worker still alive
    canary: Arc<()>,
}

impl MonitorService {
    pub fn new(config: MonitorConfig, bc_chan: BlockRequestSender) -> Self {
        let worker = MonitorWorker::new(config, bc_chan);

        MonitorService {
            worker: Some(worker),
            handler: None,
            canary: Arc::new(()),
        }
    }

    /// Start monitor service if not already running
    pub fn start(&mut self, addr: String, file: String) {
        debug!("Starting MONITOR service");

        let mut worker = match self.worker.take() {
            Some(worker) => worker,
            None => {
                warn!("Service was already running");
                return;
            }
        };

        let mut canary = Arc::clone(&self.canary);
        let handle = thread::spawn(move || {
            let _ = Arc::get_mut(&mut canary);
            worker.run(addr, file); // it was run_sync() in bridge
            worker
        });
        self.handler = Some(handle)
    }

    /// Stop monitor service
    /// TODO
    pub fn stop(&mut self) {
        debug!("Stopping MONITOR service (TODO)")
    }

    /// Check if monitor is running
    pub fn is_running(&self) -> bool {
        Arc::strong_count(&self.canary) == 2
    }
}

//#[cfg(test)]
//mod test {
//    todo!();
//}
