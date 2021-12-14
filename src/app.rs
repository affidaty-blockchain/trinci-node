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

use serde::{Deserialize, Serialize};
use trinci_core::{
    base::{serialize::rmp_deserialize, Mutex},
    blockchain::{BlockConfig, BlockRequestSender, BlockService, Event, Message},
    bridge::{BridgeConfig, BridgeService},
    crypto::{ed25519::KeyPair as ed25519KeyPair, ed25519::PublicKey as ed25519PublicKey, KeyPair},
    db::RocksDb,
    p2p::{service::PeerConfig, PeerService},
    rest::{RestConfig, RestService},
    wm::{WasmLoader, WmLocal},
    Error, ErrorKind, Transaction,
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
    pub keypair: KeyPair,
    /// p2p Keypair placeholder
    pub p2p_public_key: ed25519PublicKey,
    /// Bootstrap path
    pub bootstrap_path: String,
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

use trinci_core::blockchain::IsValidator;

// All nodes are validator for the first block
fn is_validator_function_temporary(value: bool) -> impl IsValidator {
    move |_account_id| Ok(value)
}

/// Method to check if the node is a current validator
fn is_validator_function(chan: BlockRequestSender) -> impl IsValidator {
    move |account_id: String| {
        let mut validators_key = String::from("blockchain:validators:");
        validators_key.push_str(&account_id);

        let req = Message::GetAccountRequest {
            id: SERVICE_ACCOUNT_ID.to_string(),
            data: vec![validators_key],
        };
        let res_chan = chan.send_sync(req).unwrap();
        match res_chan.recv_sync() {
            Ok(Message::GetAccountResponse { acc: _, mut data }) => {
                if data.is_empty() || data[0].is_none() {
                    Err(Error::new_ext(
                        ErrorKind::ResourceNotFound,
                        "data not found",
                    ))
                } else {
                    rmp_deserialize::<bool>(&data[0].take().unwrap())
                }
            }
            Ok(Message::Exception(err)) => Err(err),
            _ => Err(Error::new(ErrorKind::Other)),
        }
    }
}

/// Smart contracts loader that is used during the bootstrap phase.
/// This loader unconditionally loads the "bootstrap" contract and ignores the
/// requested contract hash.
fn bootstrap_loader(bootstrap_bin: Vec<u8>) -> impl WasmLoader {
    move |_hash| Ok(bootstrap_bin.to_owned())
}

// Smart contracts loader that loads the binaries that were registered in the
// "service" account.
fn blockchain_loader(chan: BlockRequestSender) -> impl WasmLoader {
    move |hash| {
        // This is the path followed during normal operational stage.

        let mut code_key = String::from("contracts:code:");
        code_key.push_str(&hex::encode(hash));

        let req = Message::GetAccountRequest {
            id: SERVICE_ACCOUNT_ID.to_string(),
            data: vec![code_key],
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
                    debug!(
                        "Bootstrap is over, switching to a better loader and validator check..."
                    );
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

#[derive(Debug, Serialize, Deserialize)]
struct BlockchainSettings {
    network_name: String,
    block_threshold: usize,
    block_timeout: u16,
}

// Load the boostrap struct from file, panic if something goes wrong
fn load_bootstrap_struct_from_file(path: &str) -> Bootstrap {
    let mut bootstrap_file = std::fs::File::open(path).expect("bootstrap file not found");
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut bootstrap_file, &mut buf).expect("loading bootstrap");

    rmp_deserialize::<Bootstrap>(&buf).expect("bootstrap file malformed")
}
#[derive(Serialize, Deserialize)]
struct Bootstrap {
    // Binary bootstrap.wasm
    bin: Vec<u8>,
    // Vec of transaction for the genesis block
    txs: Vec<Transaction>,
}

// If this panics, it panics early at node boot. Not a big deal.
fn load_config_from_service(chan: &BlockRequestSender) -> BlockchainSettings {
    let res_chan = chan
        .send_sync(Message::GetAccountRequest {
            id: SERVICE_ACCOUNT_ID.to_string(),
            data: vec!["blockchain:settings".to_string()],
        })
        .unwrap();
    match res_chan.recv_sync() {
        Ok(Message::GetAccountResponse { acc: _, data }) => {
            let data = data.get(0).unwrap().as_ref().unwrap(); // The unwrap propagates the panic!
            match rmp_deserialize::<BlockchainSettings>(data) {
                Ok(value) => value,
                Err(_) => panic!("Settings deserialization failure"),
            }
        }
        Ok(Message::Exception(err)) => match err.kind {
            ErrorKind::ResourceNotFound => panic!("Resource not found"),
            _ => panic!("Unexpected error: {}", err),
        },
        Ok(res) => panic!("Unexpected response from blockchain: {:?}", res),
        Err(err) => panic!("Channel error: {:?}", err),
    }
}

impl App {
    /// Create a new Application instance.
    pub fn new(config: Config, keypair: KeyPair) -> Self {
        let temporary_bootstrap_loader = bootstrap_loader(vec![]);
        let wm = WmLocal::new(temporary_bootstrap_loader, config.wm_cache_max);
        let db = RocksDb::new(&config.db_path);

        let block_config = BlockConfig {
            threshold: config.block_threshold,
            timeout: config.block_timeout,
            network: config.network.clone(),
        };

        let is_validator = is_validator_function_temporary(true);

        let block_svc = BlockService::new(
            &keypair.public_key().to_account_id(),
            is_validator,
            block_config,
            db,
            wm,
        );
        let chan = block_svc.request_channel();

        let rest_config = RestConfig {
            addr: config.rest_addr.clone(),
            port: config.rest_port,
        };
        let rest_svc = RestService::new(rest_config, chan.clone());

        let p2p_keypair = ed25519KeyPair::from_random();
        let p2p_public_key: ed25519PublicKey = p2p_keypair.public_key();

        let p2p_config = PeerConfig {
            addr: config.p2p_addr.clone(),
            port: config.p2p_port,
            network: config.network.clone(),
            bootstrap_addr: config.p2p_bootstrap_addr.clone(),
            p2p_keypair: Some(p2p_keypair),
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
            p2p_public_key,
            bootstrap_path: config.bootstrap_path,
        }
    }

    // Set the block service config
    fn set_block_service_config(&mut self, config: BlockchainSettings) {
        self.block_svc.stop();
        self.block_svc.set_block_config(
            config.network_name,
            config.block_threshold,
            config.block_timeout,
        );
        self.block_svc.start();
    }

    // Load the config from the SERVICE data and store it in the block_service
    fn set_config_from_service(&mut self, chan: &BlockRequestSender) {
        let config = load_config_from_service(chan);

        self.set_block_service_config(config);
    }

    // Set is_validator closure for block service
    fn set_block_service_is_validator(&mut self, is_validator: impl IsValidator) {
        self.block_svc.stop();
        self.block_svc.set_validator(is_validator);
        self.block_svc.start();
    }

    // Insert the initial transactions in the pool
    fn put_txs_in_the_pool(&mut self, txs: Vec<Transaction>) {
        self.block_svc.stop();
        self.block_svc.put_txs(txs);
        self.block_svc.start();
    }

    /// Starts the blockchain service to receive messages from the bootstrap procedure.
    /// Spawn a temporary thread that takes care of "service" account creation.
    /// Once that the service account is created, the thread takes care to set the
    /// main smart contracts loader within the wasm machine.
    pub fn start(&mut self) {
        self.block_svc.start();

        let chan = self.block_svc.request_channel();
        if is_service_present(&chan) {
            self.set_config_from_service(&chan);

            let loader = blockchain_loader(chan.clone());
            self.block_svc.wm_arc().lock().set_loader(loader);
        } else {
            let wm = self.block_svc.wm_arc();

            // Load the Boostrap Struct from file
            let bootstrap = load_bootstrap_struct_from_file(&self.bootstrap_path);

            let bootstrap_loader = bootstrap_loader(bootstrap.bin);
            self.block_svc.wm_arc().lock().set_loader(bootstrap_loader);

            self.set_block_service_config(BlockchainSettings {
                network_name: "bootstrap".to_string(),
                block_threshold: bootstrap.txs.len(),
                block_timeout: 2, // The genesis block will be executed after this timeout and not with block_threshold transactions in the pool // FIXME
            });

            self.put_txs_in_the_pool(bootstrap.txs);

            bootstrap_monitor(chan.clone(), wm);

            self.set_config_from_service(&chan.clone());
        }

        let is_validator = is_validator_function(chan.clone());

        self.set_block_service_is_validator(is_validator);

        warn!("Starting the services");

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

#[test]
fn read_bootstrap_bin() {
    let mut bootstrap_file = std::fs::File::open("./bootstrap.bin").unwrap();
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut bootstrap_file, &mut buf).expect("loading bootstrap");

    let bootstrap = rmp_deserialize::<Bootstrap>(&buf);

    assert!(bootstrap.is_ok());
}

#[test]
#[ignore = "this is a temporary way to create bootstrap.bin"]
fn create_bootstrap_bin() {
    let mut bootstrap_file = std::fs::File::open("./bootstrap.wasm").unwrap();
    let mut bootstrap = Vec::new();
    std::io::Read::read_to_end(&mut bootstrap_file, &mut bootstrap).expect("loading bootstrap");

    let mut bootstrap_init_file = std::fs::File::open("../trinci-cli/tx1.bin").unwrap();

    let mut bootstrap_init_bin = Vec::new();
    std::io::Read::read_to_end(&mut bootstrap_init_file, &mut bootstrap_init_bin)
        .expect("loading bootstrap init bin");

    println!("{}", hex::encode(&bootstrap_init_bin));

    let tx1: Transaction = rmp_deserialize(&bootstrap_init_bin).unwrap();

    let mut register_asset_file = std::fs::File::open("../trinci-cli/tx2.bin").unwrap();

    let mut register_asset_bin = Vec::new();
    std::io::Read::read_to_end(&mut register_asset_file, &mut register_asset_bin)
        .expect("loading register asset bin");
    let tx2: Transaction = rmp_deserialize(&register_asset_bin).unwrap();

    let txs = vec![tx1, tx2];

    let bootstrap_bin = Bootstrap {
        bin: bootstrap,
        txs,
    };

    let bootstrap_buf = trinci_core::base::serialize::rmp_serialize(&bootstrap_bin).unwrap();

    std::fs::write("bootstrap.bin", bootstrap_buf).unwrap();
}
