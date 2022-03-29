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

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use trinci_core::blockchain::{BlockRequestSender, Event, Message};

// Temporary structure to keep track for executed transactions per second.
#[derive(Default)]
struct Tracer {
    begin: Duration,
    txs: usize,
}

impl Tracer {
    pub fn new() -> Tracer {
        Tracer::default()
    }

    // Ugly method to keep track of transactions per second.
    // This is only meant to be used during stress tests.
    fn update(&mut self, height: usize, count: usize) {
        if self.txs == 0 {
            self.begin = SystemTime::now().duration_since(UNIX_EPOCH).unwrap(); // Safe
        }
        self.txs += count;
        let delta = SystemTime::now().duration_since(UNIX_EPOCH).unwrap() - self.begin; // Safe
        let tps = self.txs as f64 / delta.as_secs() as f64;
        info!(
            "[tracer] height: {}, block-txs: {}, total-txs: {}, ~tps: {}",
            height, count, self.txs, tps
        );
    }
}

pub fn run(tx_chan: BlockRequestSender) {
    let mut tracer = Tracer::new();

    let msg = Message::Subscribe {
        id: "tracer".to_owned(),
        events: Event::BLOCK,
    };

    // Get subscription channel.
    let rx_chan = match tx_chan.send_sync(msg) {
        Ok(chan) => chan,
        Err(_) => {
            warn!("[tracer] blockchain channel closed");
            return;
        }
    };

    loop {
        match rx_chan.recv_sync() {
            Ok(Message::GetBlockResponse { block, .. }) => {
                tracer.update(block.data.height as usize, block.data.size as usize);
            }
            Ok(res) => {
                info!("[tracer] Subscribe response: {:?}", res);
            }
            Err(_) => {
                warn!("[tracer] blockchain channel closed");
                break;
            }
        }
    }
}
