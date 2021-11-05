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

use crate::{config::Config, config::SERVICE_ACCOUNT_ID};
use std::sync::Arc;
use trinci_core::{
    base::Mutex,
    blockchain::{BlockConfig, BlockRequestSender, BlockService, Event, Message},
    bridge::{BridgeConfig, BridgeService},
    crypto::KeyPair,
    db::RocksDb,
    p2p::{service::PeerConfig, PeerService},
    rest::{RestConfig, RestService},
    wm::{WasmLoader, WmLocal},
    Error, ErrorKind,
};

/// Application context.
pub struct App {
    /// Block service context.
    pub block_svc: BlockService<RocksDb, WmLocal>,
    /// Rest service context.
    pub rest_svc: RestService,
    /// Peer2Peer service context.
    pub p2p_svc: PeerService,
    /// Bridge service context.
    pub bridge_svc: BridgeService,
    /// Keypair placeholder.
    pub keypair: Option<KeyPair>,
}

// If this panics, it panics early at node boot. Not a big deal.
fn is_service_present(chan: &BlockRequestSender) -> bool {
    let res_chan = chan
        .send_sync(Message::GetAccountRequest {
            id: SERVICE_ACCOUNT_ID.to_string(),
            data: vec![],
        })
        .unwrap();
    match res_chan.recv_sync() {
        Ok(Message::GetAccountResponse { acc: _, data: _ }) => true,
        Ok(Message::Exception(err)) => match err.kind {
            ErrorKind::ResourceNotFound => false,
            _ => panic!("Unexpected error: {}", err),
        },
        Ok(res) => panic!("Unexpected response from blockchain: {:?}", res),
        Err(err) => panic!("Channel error: {:?}", err),
    }
}

/// Smart contracts loader that is used during the bootstrap phase.
/// This loader unconditionally loads the "bootstrap" contract and ignores the
/// requested contract hash.
fn bootstrap_loader(bootstrap_path: String) -> impl WasmLoader {
    move |_hash| std::fs::read(&bootstrap_path).map_err(|err| Error::new_ext(ErrorKind::Other, err))
}

// Smart contracts loader that loads the binaries that were registered in the
// "service" account.
fn blockchain_loader(chan: BlockRequestSender) -> impl WasmLoader {
    move |hash| {
        // This is the path followed during normal operational stage.
        let req = Message::GetAccountRequest {
            id: SERVICE_ACCOUNT_ID.to_string(),
            data: vec![hex::encode(hash)],
        };
        let res_chan = chan.send_sync(req).unwrap();
        match res_chan.recv_sync() {
            Ok(Message::GetAccountResponse { acc: _, mut data }) => {
                if data.is_empty() || data[0].is_none() {
                    Err(Error::new_ext(
                        ErrorKind::ResourceNotFound,
                        "smart contract not found",
                    ))
                } else {
                    Ok(data[0].take().unwrap())
                }
            }
            Ok(Message::Exception(err)) => Err(err),
            _ => Err(Error::new(ErrorKind::Other)),
        }
    }
}

fn bootstrap_monitor(chan: BlockRequestSender, wm: Arc<Mutex<WmLocal>>) {
    debug!("Bootstrap procedure started");

    let res_chan = chan
        .send_sync(Message::Subscribe {
            id: "bootstrap".to_string(),
            events: Event::BLOCK,
        })
        .unwrap();
    loop {
        match res_chan.recv_sync() {
            Ok(Message::GetBlockResponse { .. }) => {
                if is_service_present(&chan) {
                    debug!("Bootstrap is over, switching to a better loader...");
                    let loader = blockchain_loader(chan.clone());
                    wm.lock().set_loader(loader);
                    break;
                } else {
                    warn!("Block constructed but 'service' account is not yet active");
                }
            }
            Ok(res) => warn!("Bootstrap unexpected message: {:?}", res),
            Err(err) => error!("Channel error: {}", err),
        }
    }
    chan.send_sync(Message::Unsubscribe {
        id: "bootstrap".to_string(),
        events: Event::BLOCK,
    })
    .unwrap();
}

impl App {
    /// Create a new Application instance.
    pub fn new(config: Config, keypair: Option<KeyPair>) -> Self {
        let bootstrap_loader = bootstrap_loader(config.bootstrap_path.clone());
        let wm = WmLocal::new(bootstrap_loader, config.wm_cache_max);
        let db = RocksDb::new(&config.db_path);

        let block_config = BlockConfig {
            validator: config.validator,
            threshold: config.block_threshold,
            timeout: config.block_timeout,
            network: config.network.clone(),
        };
        let block_svc = BlockService::new(block_config, db, wm);
        let chan = block_svc.request_channel();

        let rest_config = RestConfig {
            addr: config.rest_addr.clone(),
            port: config.rest_port,
        };
        let rest_svc = RestService::new(rest_config, chan.clone());

        let p2p_config = PeerConfig {
            addr: config.p2p_addr.clone(),
            port: config.p2p_port,
            network: config.network.clone(),
            bootstrap_addr: config.p2p_bootstrap_addr.clone(),
        };
        let p2p_svc = PeerService::new(p2p_config, chan.clone());

        let bridge_config = BridgeConfig {
            addr: config.bridge_addr,
            port: config.bridge_port,
        };
        let bridge_svc = BridgeService::new(bridge_config, chan);

        App {
            block_svc,
            rest_svc,
            p2p_svc,
            bridge_svc,
            keypair,
        }
    }

    /// Starts the blockchain service to receive messages from the bootstrap procedure.
    /// Spawn a temporary thread that takes care of "service" account creation.
    /// Once that the service account is created, the thread takes care to set the
    /// main smart contracts loader within the wasm machine.
    pub fn start(&mut self) {
        self.block_svc.start();

        let chan = self.block_svc.request_channel();
        if is_service_present(&chan) {
            let loader = blockchain_loader(chan);
            self.block_svc.wm_arc().lock().set_loader(loader);
        } else {
            let wm = self.block_svc.wm_arc();
            std::thread::spawn(move || {
                bootstrap_monitor(chan, wm);
            });
        }

        self.rest_svc.start();
        self.p2p_svc.start();
        self.bridge_svc.start();
    }

    pub fn park(&mut self) {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let mut stop = false;
            if !self.block_svc.is_running() {
                error!("Blockchain service is not running");
                stop = true;
            }
            if !self.rest_svc.is_running() {
                error!("Rest service is not running");
                stop = true;
            }
            // if !self.p2p_svc.is_running() {
            //     error!("P2P service is not running");
            //     stop = true;
            // }
            if !self.bridge_svc.is_running() {
                error!("Bridge service is not running");
                stop = true;
            }
            if stop {
                self.block_svc.stop();
                self.rest_svc.stop();
                self.p2p_svc.stop();
                self.bridge_svc.stop();
                break;
            }
        }
        println!("Something bad happened, stopping the application");
    }
}
