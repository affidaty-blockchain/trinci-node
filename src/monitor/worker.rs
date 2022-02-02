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
use std::{
    collections::BTreeMap, fmt::Display, fs::File, io::Write, thread::sleep, time::Duration,
};
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
    #[allow(dead_code)] // FIXME
    Validator,
}

#[derive(Serialize)]
pub struct Status {
    /// public key associated with the node
    pub public_key: String,
    /// public key associated with the node for p2p network
    pub nw_public_key: String,
    /// ip entry point to contact the node (local)
    pub ip_endpoint: Option<String>,
    /// ip seen from the extern
    pub pub_ip: Option<String>,
    /// node's role
    pub role: NodeRole,
    /// partial network config that reside in the bootstrap
    pub nw_config: NetworkConfig,
    /// core version held by the node
    pub core_version: String,
    /// last node's block
    pub last_block: Option<LastBlock>,
    /// structure that holds some infomration about the unconfirmed tx queue
    pub unconfirmed_pool: Option<UnconfirmedPool>,
    /// infos reuarding the p2p config
    pub p2p_info: P2pInfo,
    /// seed
    pub seed: u64,
    // TODO
    //rcv_message_in_window: T,
}

// due to server interaction the Monitor server
// structure needs this names as field
/// It holds the node informations
#[derive(Serialize)]
#[allow(non_snake_case)]
pub struct MonitorConfig {
    pub(crate) nodeID: String,
    pub(crate) data: Status,
}

pub struct MonitorWorker {
    config: MonitorConfig,
    bc_chan: BlockRequestSender,
}

impl MonitorWorker {
    pub fn new(config: MonitorConfig, bc_chan: BlockRequestSender) -> Self {
        MonitorWorker { config, bc_chan }
    }

    /// Updates node status
    fn update(&mut self, block: Option<Block>, unconfirmed_pool: Option<UnconfirmedPool>) {
        self.config.data.unconfirmed_pool = unconfirmed_pool;

        if let Some(block) = block {
            let hash = block.hash(HashAlgorithm::Sha256);
            let last_block = LastBlock { block, hash };
            self.config.data.last_block = Some(last_block);
        }

        // retireve seed
        let request = Message::GetSeedRequest;
        let rx_chan = match self.bc_chan.send_sync(request) {
            Ok(rx_chan) => rx_chan,
            Err(_error) => {
                warn!("[monitor] blockchain channel closed");
                return;
            }
        };
        match rx_chan.recv_sync() {
            Ok(Message::GetSeedRespone(seed)) => self.config.data.seed = seed,
            Ok(res) => {
                warn!("[monitor] unexpected message {:?}", res);
            }
            Err(_error) => {
                warn!("[monitor] blockchain channel closed");
            }
        }
    }

    /// Send json structure containing node status to the `addr`
    fn send_update(&mut self, addr: String) {
        let request = match serde_json::to_string(&self.config) {
            Ok(request) => request,
            Err(_error) => {
                warn!("[monitor] error in serializing monitor structure");
                return;
            }
        };

        debug!("{}", request);

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
            Ok(_response) => debug!("[monitor] update sended"),
            Err(error) => warn!("[monitor] {:?}", error),
        }
    }

    /// Saves node status in a human readable format in the `file` specified
    fn save_update(&mut self, file: String) {
        // write structure in file
        let mut columns = BTreeMap::new();
        let column_field = Column {
            header: "field".into(),
            align: Align::Left,
            max_width: 100,
        };
        let column_vals = Column {
            header: "value".into(),
            align: Align::Center,
            max_width: 100,
        };
        columns.insert(0, column_field);
        columns.insert(1, column_vals);
        let mut ascii_table = AsciiTable {
            max_width: 100,
            columns,
        };
        ascii_table.max_width = 100;

        // data preparation
        let role = match &self.config.data.role {
            NodeRole::Ordinary => "ordinary",
            NodeRole::Validator => "validator",
        };

        let ip_endpoint = match &self.config.data.ip_endpoint {
            Some(ip) => ip.clone(),
            None => String::from("None"),
        };

        let pub_ip = match &self.config.data.pub_ip {
            Some(ip) => ip.clone(),
            None => String::from("None"),
        };

        let data: Vec<Vec<&dyn Display>> = vec![
            vec![&"public key", &self.config.data.public_key],
            vec![&"network public key", &self.config.data.nw_public_key],
            vec![&"public IP", &pub_ip],
            vec![&"IP end point", &ip_endpoint],
            vec![&"role", &role],
            vec![&"core version", &self.config.data.core_version],
        ];
        let mut file = File::create(file).unwrap();
        file.write_all(b"\nnode id:\n")
            .is_err()
            .then(|| warn!("[monitor] error in file write"));
        file.write_all(self.config.nodeID.as_bytes())
            .is_err()
            .then(|| warn!("[monitor] error in file write"));
        file.write_all(b"\n\nnode info\n")
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
            vec![&"network name", &self.config.data.nw_config.name],
            //vec![&"network id", &network_id], todo!()
            vec![
                &"block threshold",
                &self.config.data.nw_config.block_threshold,
            ],
            vec![&"block timeout", &self.config.data.nw_config.block_timeout],
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
        let bootstrap_addr = match &self.config.data.p2p_info.p2p_bootstrap_addr {
            Some(addr) => addr.clone(),
            None => String::from("None"),
        };

        let p2p_data: Vec<Vec<&dyn Display>> = vec![
            vec![&"p2p address", &self.config.data.p2p_info.p2p_addr],
            vec![&"p2p port", &self.config.data.p2p_info.p2p_port],
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
        match &self.config.data.last_block {
            Some(last_block) => {
                // data preparation

                let last_block_hash = hex::encode(last_block.hash.as_bytes());
                let prev_hash = hex::encode(last_block.block.data.prev_hash.as_bytes());
                let txs_hash = hex::encode(last_block.block.data.txs_hash.as_bytes());
                let rxs_hash = hex::encode(last_block.block.data.rxs_hash.as_bytes());
                let state_hash = hex::encode(last_block.block.data.state_hash.as_bytes());

                let block_data: Vec<Vec<&dyn Display>> = vec![
                    vec![&"hash", &last_block_hash],
                    vec![&"height", &last_block.block.data.height],
                    vec![&"size", &last_block.block.data.size],
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
                file.write_all(b"None\n")
                    .is_err()
                    .then(|| warn!("[monitor] error in file write"));
            }
        }
        // ----------------------
        // unconfirmed pool handling
        file.write_all(b"\nunconfirmed pool\n")
            .is_err()
            .then(|| warn!("[monitor] error in file write"));
        match &self.config.data.unconfirmed_pool {
            Some(pool) => {
                let hash = hex::encode(pool.hash.hash_value());
                let pool_data: Vec<Vec<&dyn Display>> =
                    vec![vec![&"hash", &hash], vec![&"lenght", &pool.size]];
                file.write_all(ascii_table.format(pool_data).as_bytes())
                    .is_err()
                    .then(|| warn!("[monitor] error in file write"));
            }
            None => {
                file.write_all(b"None\n")
                    .is_err()
                    .then(|| warn!("[monitor] error in file write"));
            }
        }

        let seed: Vec<Vec<&dyn Display>> = vec![vec![&"seed", &self.config.data.seed]];
        file.write_all(ascii_table.format(seed).as_bytes())
            .is_err()
            .then(|| warn!("[monitor] error in file write"));

        debug!("[monitor] update saved");
    }

    /// Run monitor, it saves every 5 minutes the node status in `file`
    /// and sends a his json representation to `addr`
    pub fn run(&mut self, addr: String, file: String) {
        debug!("[monitor] running, monitor data updated every 5 min");

        // retireve network id
        let request = Message::GetNetworkIdRequest;
        let rx_chan = match self.bc_chan.send_sync(request) {
            Ok(rx_chan) => rx_chan,
            Err(_error) => {
                warn!("[monitor] blockchain channel closed");
                return;
            }
        };
        match rx_chan.recv_sync() {
            Ok(Message::GetNetworkIdResponse(info)) => self.config.data.nw_config.name = info,
            Ok(res) => {
                warn!("[monitor] unexpected message {:?}", res);
            }
            Err(_error) => {
                warn!("[monitor] blockchain channel closed");
            }
        }

        loop {
            sleep(Duration::new(60 * 5, 0));

            let request = Message::GetCoreStatsRequest;
            let rx_chan = match self.bc_chan.send_sync(request) {
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
                        self.update(info.2, unconfirmed_pool);
                    } else {
                        self.update(info.2, None)
                    }

                    self.send_update(addr.clone());
                    self.save_update(file.clone());
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
}
