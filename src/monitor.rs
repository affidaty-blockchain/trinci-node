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

// collect statics infos (in new)
// listen on port
// add station to stations list
// each t seconds:
//              by using channel to blockchain dispatcher
//              request dynamic infos to core via GetCoreStatsRequest to blockchain
//              recive infos via GetCoreStatsresponse
//              send infos to all the stations
use ascii_table::{Align, AsciiTable, Column};
use isahc::{Request, RequestExt};
use serde::Serialize;
use serde_json;
use std::{fmt::Display, fs::File, io::Write, str::from_utf8, thread::sleep, time::Duration};
#[cfg(feature = "monitor")]
use trinci_core::{
    blockchain::BlockRequestSender,
    crypto::{Hash, HashAlgorithm, Hashable},
    Block, Message,
};

/// structure to track node information
#[derive(Serialize)]
/// structure that holds the hash of the unconfirmed transaction queue and it's dimension
pub struct UnconfirmedPool {
    hash: Hash,
    size: usize,
}

#[derive(Serialize)]
/// structure that holds the last block recieved by the node and its hash
pub struct LastBlock {
    block: Block,
    hash: Hash,
}

#[derive(Serialize)]
pub struct P2pInfo {
    pub p2p_addr: String,
    /// P2p service tcp port.
    pub p2p_port: u16,
    /// P2P service bootstrap address.
    pub p2p_bootstrap_addr: Option<String>,
}

#[derive(Serialize)]
pub struct NetworkConfig {
    pub name: String,
    // it should be the bootstrap hash
    //network_id: Hash, todo!()
    pub block_threshold: usize,
    pub block_timeout: u16,
}

#[derive(Serialize)]
pub enum NodeRole {
    Ordinary,
    Validator,
}

#[derive(Serialize)]
pub struct Status {
    /// public key associated with the node
    pub public_key: String,
    /// public key associated with the node for p2p network
    pub nw_public_key: String,
    /// ip entry point to contact the node
    pub ip_endpoint: Option<String>,
    /// node's role
    pub role: NodeRole,
    /// partial netowrk config that reside in the bootstrap
    pub nw_config: NetworkConfig,
    /// core version held by the node
    pub core_version: String,
    /// last node's block
    pub last_block: Option<LastBlock>,
    /// structure that holds some infomration about the unconfirmed tx queue
    pub unconfirmed_pool: Option<UnconfirmedPool>,
    /// infos reuarding the p2p config
    pub p2p_info: P2pInfo,
    // TODO
    //rcv_message_in_window: T,
}

#[derive(Serialize)]
pub struct Monitor {
    node_id: String,
    node_status: Status,
}

impl Monitor {
    pub fn new(node_id: String, node_status: Status) -> Monitor {
        Monitor {
            node_id,
            node_status,
        }
    }

    // function to update node status
    fn update(&mut self, block: Option<Block>, unconfirmed_pool: Option<UnconfirmedPool>) {
        self.node_status.unconfirmed_pool = unconfirmed_pool;

        match block {
            Some(block) => {
                let hash = block.hash(HashAlgorithm::Sha256);
                let last_block = LastBlock { block, hash };
                self.node_status.last_block = Some(last_block);
            }
            None => {}
        }
    }
}

fn send_update(monitor: &mut Monitor, addr: &String) {
    let request = match serde_json::to_vec(&monitor) {
        Ok(request) => request,
        Err(_error) => {
            warn!("[monitor] error in serializing monitor structure");
            return;
        }
    };

    let response = match Request::post(addr)
        .header("content-type", "application/json")
        .body(request)
    {
        Ok(response) => response,
        Err(_error) => {
            warn!("[monitor] error in sending POST");
            return;
        }
    };

    match response.send() {
        Ok(_) => println!("update sended"),
        Err(error) => warn!("[monitor] {:?}", error),
    }
}

fn save_update(monitor: &mut Monitor, file: &String) {
    // write structure in file
    let mut ascii_table = AsciiTable::default();
    ascii_table.max_width = 100;

    let mut column = Column::default();
    column.header = "field".into();
    column.align = Align::Left;
    ascii_table.columns.insert(0, column);

    let mut column = Column::default();
    column.header = "value".into();
    column.align = Align::Center;
    ascii_table.columns.insert(1, column);

    // data preparation
    let role = match &monitor.node_status.role {
        NodeRole::Ordinary => "ordinary",
        NodeRole::Validator => "validator",
    };

    let ip_endpoint = match &monitor.node_status.ip_endpoint {
        Some(ip) => ip.clone(),
        None => String::from("None").clone(),
    };

    let data: Vec<Vec<&dyn Display>> = vec![
        vec![&"public key", &monitor.node_status.public_key],
        vec![&"network public key", &monitor.node_status.nw_public_key],
        vec![&"IP end point", &ip_endpoint],
        vec![&"role", &role],
        vec![&"core version", &monitor.node_status.core_version],
    ];
    let mut file = File::create(file).unwrap();
    file.write_all(b"\nnode id:\n")
        .is_err()
        .then(|| warn!("[monitor] error in file write"));
    file.write_all(monitor.node_id.as_bytes())
        .is_err()
        .then(|| warn!("[monitor] error in file write"));
    file.write_all(b"\nnode info\n")
        .is_err()
        .then(|| warn!("[monitor] error in file write"));
    file.write_all(ascii_table.format(data).as_bytes())
        .is_err()
        .then(|| warn!("[monitor] error in file write"));

    // ----------------------
    // network data handling

    // data preparation todo!()
    //let network_id = match from_utf8(monitor.node_status.nw_config.network_id.as_bytes()){
    //    Ok(str) => str,
    //    Err(_) => "None",
    //};

    let network_data: Vec<Vec<&dyn Display>> = vec![
        vec![&"network name", &monitor.node_status.nw_config.name],
        //vec![&"network id", &network_id], todo!()
        vec![
            &"block threshold",
            &monitor.node_status.nw_config.block_threshold,
        ],
        vec![
            &"block timeout",
            &monitor.node_status.nw_config.block_timeout,
        ],
    ];
    file.write_all(b"\nnetwork info\n")
        .is_err()
        .then(|| warn!("[monitor] error in file write"));
    file.write_all(ascii_table.format(network_data).as_bytes())
        .is_err()
        .then(|| warn!("[monitor] error in file write"));

    // ----------------------
    // p2p data handling

    // data preparation
    let bootstrap_addr = match &monitor.node_status.p2p_info.p2p_bootstrap_addr {
        Some(addr) => addr.clone(),
        None => String::from("None").clone(),
    };

    let p2p_data: Vec<Vec<&dyn Display>> = vec![
        vec![&"p2p address", &monitor.node_status.p2p_info.p2p_addr],
        vec![&"p2p port", &monitor.node_status.p2p_info.p2p_port],
        vec![&"p2p bootsrap address", &bootstrap_addr],
    ];
    file.write_all(b"\np2p info\n")
        .is_err()
        .then(|| warn!("[monitor] error in file write"));
    file.write_all(ascii_table.format(p2p_data).as_bytes())
        .is_err()
        .then(|| warn!("[monitor] error in file write"));

    // ----------------------
    // last block handling
    file.write_all(b"\nlast block\n")
        .is_err()
        .then(|| warn!("[monitor] error in file write"));
    match &monitor.node_status.last_block {
        Some(last_block) => {
            // data preparation
            let last_block_hash = match from_utf8(last_block.hash.as_bytes()) {
                Ok(str) => str,
                Err(_) => "None",
            };

            let prev_hash = match from_utf8(last_block.block.prev_hash.as_bytes()) {
                Ok(str) => str,
                Err(_) => "None",
            };

            let txs_hash = match from_utf8(last_block.block.txs_hash.as_bytes()) {
                Ok(str) => str,
                Err(_) => "None",
            };

            let rxs_hash = match from_utf8(last_block.block.rxs_hash.as_bytes()) {
                Ok(str) => str,
                Err(_) => "None",
            };

            let state_hash = match from_utf8(last_block.block.state_hash.as_bytes()) {
                Ok(str) => str,
                Err(_) => "None",
            };

            let block_data: Vec<Vec<&dyn Display>> = vec![
                vec![&"hash", &last_block_hash],
                vec![&"height", &last_block.block.height],
                vec![&"size", &last_block.block.size],
                vec![&"previous hash", &prev_hash],
                vec![&"txs hash", &txs_hash],
                vec![&"rxs hash", &rxs_hash],
                vec![&"state hash", &state_hash],
            ];
            file.write_all(ascii_table.format(block_data).as_bytes())
                .is_err()
                .then(|| warn!("[monitor] error in file write"));
        }
        None => {
            file.write_all(b"None")
                .is_err()
                .then(|| warn!("[monitor] error in file write"));
        }
    }
}

pub fn run(
    monitor: &mut Monitor,
    tx_chan: BlockRequestSender,
    addr: &Option<String>,
    file: &String,
) {
    debug!("[monitor] running, waiting 5 minutes for first run!");

    loop {
        sleep(Duration::new(60 * 1, 0));

        let request = Message::GetCoreStatsRequest;
        let rx_chan = match tx_chan.send_sync(request) {
            Ok(rx_chan) => rx_chan,
            Err(_error) => {
                warn!("[monitor] blockchain channel closed");
                return;
            }
        };

        match rx_chan.recv_sync() {
            Ok(Message::GetCoreStatsResponse(info)) => {
                if info.1 > 0 {
                    let unconfirmed_pool = Some(UnconfirmedPool {
                        hash: info.0,
                        size: info.1,
                    });
                    monitor.update(info.2, unconfirmed_pool);
                } else {
                    monitor.update(info.2, None)
                }

                match addr {
                    Some(addr) => {
                        send_update(monitor, &addr);
                        debug!("[monitor] update sended");
                    }
                    None => {}
                }
                save_update(monitor, &file);
                debug!("[monitor] update saved");
            }
            Ok(res) => {
                warn!("[monitor] unexpected message {:?}", res);
            }
            Err(_error) => {
                warn!("[monitor] blockchain channel closed");
                break;
            }
        }
    }
}
