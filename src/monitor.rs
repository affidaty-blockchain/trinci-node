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
use trinci_core::{Block, Message, blockchain::BlockRequestSender, crypto::{Hash, Hashable, HashAlgorithm}};
use serde::{Serialize};
use isahc::{Request, RequestExt};
use serde_json;
use std::{io::{Write, Error}, fmt::Display, str::from_utf8, fs::File};
use ascii_table::{AsciiTable, Column, Align};

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
struct P2pInfo {
    p2p_addr: String,
    /// P2p service tcp port.
    p2p_port: u16,
    /// P2P service bootstrap address.
    p2p_bootstrap_addr: Option<String>,
}

#[derive(Serialize)]
struct NetworkConfig {
    name: String,
    network_id: Hash,
    block_threshold: i32,
    block_timeout: i32,
}

#[derive(Serialize)]
enum NodeRole {
    Ordinary,
    Validator,
}

#[derive(Serialize)]
pub struct Status {
    /// public key associated with the node
    public_key: Vec<u8>,
    /// public key associated with the node for p2p network
    nw_public_key: Vec<u8>,
    /// ip entry point to contact the node
    ip_endpoint: Option<String>,
    /// node's role
    // mabye a enum with different type may be more appropriate
    role: NodeRole,
    /// partial netowrk config that reside in the bootstrap
    nw_config: NetworkConfig,
    /// core version held by the node
    core_version: String,
    /// last node's block
    last_block: Option<LastBlock>,
    /// structure that holds some infomration about the unconfirmed tx queue
    unconfirmed_pool: Option<UnconfirmedPool>,
    /// infos reuarding the p2p config
    p2p_info: P2pInfo,
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
                let last_block = LastBlock {
                    block,
                    hash,
                };
                self.node_status.last_block = Some(last_block);
            },
            None => {

            },
        }
    }

}

fn send_update(monitor: &mut Monitor, addr: &String) {
    let request = match serde_json::to_vec(&monitor){
        Ok(request) => request,
        Err(_error) => {       
            warn!("[monitor] error in serializing monitor structure");
            return;
        },
    };

    let response = match Request::post(addr)
        .header("content-type", "application/json")
        .body(request) {
            Ok(response) => response,
            Err(_error) => {       
                warn!("[monitor] error in sending POST");
                return;
            },
        };
        
    match response.send(){
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
     
    let role = match  monitor.node_status.role {
        Ordinary => {"ordinary"},
        Validator => {"validator"},
    };

    let ip_endpoint = match monitor.node_status.ip_endpoint {
        Some(ip) => ip,
        None => String::from("None"),
    };

    let data: Vec<Vec<&dyn Display>> = vec![
        vec![&"public key", &from_utf8(&monitor.node_status.public_key.to_vec().clone()).unwrap()],
        vec![&"network public key", &from_utf8(&monitor.node_status.nw_public_key.to_vec().clone()).unwrap()],
        vec![&"IP end point", &ip_endpoint],
        vec![&"role", &role],
        vec![&"core version", &monitor.node_status.core_version],
    ];
    let mut file = File::create(file).unwrap();
    file.write_all("\nnode info\n");    
    file.write_all(ascii_table.format(data).as_bytes());

    // TODO: end table

}

pub fn run(monitor: &mut Monitor, tx_chan: BlockRequestSender, addr: String, file: String) {
    
    let request= Message::GetCoreStatsRequest;
    let rx_chan = match tx_chan.send_sync(request) {
        Ok(rx_chan) => rx_chan,
        Err(_error) => {
                warn!("[monitor] blockchain channel closed");
                return;
            },
    };


    loop {
        // TODO: sleep some time
        
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

                send_update(monitor, &addr);
                save_update(monitor, &file);
            },
            Ok(res) => {
                warn!("[monitor] unexpected message {:?}", res);
            },
            Err(_error) => {
                warn!("[monitor] blockchain channel closed");
                break;
            },
        }
    }
}