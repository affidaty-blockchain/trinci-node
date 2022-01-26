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

use crate::monitor;
use crate::monitor::service::MonitorService;
use crate::monitor::worker::MonitorConfig;
use crate::{config::Config, config::SERVICE_ACCOUNT_ID};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use trinci_core::base::BlockchainSettings;
use trinci_core::crypto::{Hash, HashAlgorithm};

use trinci_core::{
    base::{
        serialize::{rmp_deserialize, rmp_serialize},
        Mutex, RwLock,
    },
    blockchain::{BlockConfig, BlockRequestSender, BlockService, Event, Message},
    bridge::{BridgeConfig, BridgeService},
    crypto::{ed25519::KeyPair as ed25519KeyPair, ed25519::PublicKey as ed25519PublicKey, KeyPair},
    db::{Db, RocksDb, RocksDbFork},
    p2p::{service::PeerConfig, PeerService},
    rest::{RestConfig, RestService},
    wm::{WasmLoader, Wm, WmLocal},
    Error, ErrorKind, Transaction,
};
/// Application context.
pub struct App {
    /// Block service context.
    pub block_svc: Arc<Mutex<BlockService<RocksDb, WmLocal>>>,
    /// Rest service context.
    pub rest_svc: RestService,
    /// Peer2Peer service context.
    pub p2p_svc: Arc<Mutex<PeerService>>,
    /// Bridge service context.
    pub bridge_svc: BridgeService,
    /// Monitor service context.
    pub monitor_svc: Option<MonitorService>,
    /// Keypair placeholder.
    pub keypair: Arc<KeyPair>,
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
fn is_validator_function_call(
    wm: Arc<Mutex<dyn Wm>>,
    db: Arc<RwLock<dyn Db<DbForkType = RocksDbFork>>>,
) -> impl IsValidator {
    move |account_id: String| {
        let args = rmp_serialize(&account_id)?;

        let mut db = db.write().fork_create();

        let mut events = Vec::new();

        let res = wm.lock().call(
            &mut db,
            42,
            "",
            "",
            SERVICE_ACCOUNT_ID,
            "",
            None,
            "is_validator",
            &args,
            &mut events,
        )?;

        rmp_deserialize(&res)
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

// Calculate the network name from the bootstrap hash
fn calculate_network_name(data: &[u8]) -> String {
    let hash = Hash::from_data(HashAlgorithm::Sha256, data);
    bs58::encode(hash).into_string()
}

// Load the boostrap struct from file, panic if something goes wrong
fn load_bootstrap_struct_from_file(path: &str) -> (String, Vec<u8>, Vec<Transaction>) {
    let mut bootstrap_file = std::fs::File::open(path).expect("bootstrap file not found");

    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut bootstrap_file, &mut buf).expect("loading bootstrap");

    match rmp_deserialize::<Bootstrap>(&buf) {
        Ok(bs) => (calculate_network_name(&buf), bs.bin, bs.txs),
        Err(_) => (calculate_network_name(&buf), buf, vec![]),
    }
}
#[derive(Serialize, Deserialize)]
struct Bootstrap {
    // Binary bootstrap.wasm
    bin: Vec<u8>,
    // Vec of transaction for the genesis block
    txs: Vec<Transaction>,
}

// If this panics, it panics early at node boot. Not a big deal.
// This should be called only once after the genesis block
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

        let keypair = Arc::new(keypair);

        let block_config = BlockConfig {
            threshold: config.block_threshold,
            timeout: config.block_timeout,
            network: config.network.clone(),
            keypair: keypair.clone(),
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
            network: Mutex::new(config.network.clone()),
            bootstrap_addr: config.p2p_bootstrap_addr.clone(),
            p2p_keypair: Some(p2p_keypair),
        };
        let p2p_svc = PeerService::new(p2p_config, chan.clone());

        let bridge_config = BridgeConfig {
            addr: config.bridge_addr,
            port: config.bridge_port,
        };
        let bridge_svc = BridgeService::new(bridge_config, chan.clone());

        #[cfg(not(feature = "monitor"))]
        let monitor_svc: Option<MonitorService> = None;
        // block chain monitor
        #[cfg(feature = "monitor")]
        let monitor_svc = {
            let nw_public_key = p2p_public_key.to_account_id();
            let public_ip = monitor::worker::get_ip();

            let node_status = monitor::worker::Status {
                public_key: keypair.public_key().to_account_id(), // check if ok
                nw_public_key,
                ip_endpoint: None,
                role: monitor::worker::NodeRole::Ordinary, // FIXME
                nw_config: monitor::worker::NetworkConfig {
                    name: config.network,
                    //network_id: todo!(),
                    block_threshold: config.block_threshold,
                    block_timeout: config.block_timeout,
                },
                core_version: trinci_core::VERSION.to_string(),
                last_block: None,
                unconfirmed_pool: None,
                p2p_info: monitor::worker::P2pInfo {
                    p2p_addr: config.p2p_addr,
                    p2p_port: config.p2p_port,
                    p2p_bootstrap_addr: config.p2p_bootstrap_addr,
                },
                pub_ip: public_ip,
            };

            let monitor_config = MonitorConfig {
                nodeID: keypair.public_key().to_account_id(),
                data: node_status,
            };

            MonitorService::new(monitor_config, chan)
        };

        App {
            block_svc: Arc::new(Mutex::new(block_svc)),
            rest_svc,
            p2p_svc: Arc::new(Mutex::new(p2p_svc)),
            bridge_svc,
            p2p_public_key,
            bootstrap_path: config.bootstrap_path,
            keypair,
            monitor_svc: Some(monitor_svc),
        }
    }

    // Set the block service config
    fn set_block_service_config(&mut self, config: BlockchainSettings) {
        self.block_svc.lock().stop();
        self.block_svc.lock().set_block_config(
            config.network_name.unwrap(), // If this fails is at the very beginning
            config.block_threshold,
            config.block_timeout,
        );
        self.block_svc.lock().start();
    }

    // Load the config from the DB
    fn set_config_from_db(&mut self) -> String {
        let block_svc = self.block_svc.clone();
        let db = block_svc.lock().db_arc();
        let buf = db.read().load_configuration("blockchain:settings").unwrap(); // If this fails is at the very beginning
        let config = rmp_deserialize::<BlockchainSettings>(&buf).unwrap(); // If this fails is at the very beginning

        let network_name = config.network_name.clone().unwrap(); // If this fails is at the very beginning
        warn!("network name: {:?}", network_name);
        self.set_block_service_config(config);
        network_name
    }

    // Store the blockchain config in the DB
    fn store_config_into_db(&mut self, config: BlockchainSettings) {
        let block_svc = self.block_svc.clone();
        block_svc.lock().store_config_into_db(config);
    }

    // Set is_validator closure for block service
    fn set_block_service_is_validator(&mut self, is_validator: impl IsValidator) {
        self.block_svc.lock().stop();
        self.block_svc.lock().set_validator(is_validator);
        self.block_svc.lock().start();
    }

    // Insert the initial transactions in the pool
    fn put_txs_in_the_pool(&mut self, txs: Vec<Transaction>) {
        self.block_svc.lock().stop();
        self.block_svc.lock().put_txs(txs);
        self.block_svc.lock().start();
    }

    /// Starts the blockchain service to receive messages from the bootstrap procedure.
    /// Spawn a temporary thread that takes care of "service" account creation.
    /// Once that the service account is created, the thread takes care to set the
    /// main smart contracts loader within the wasm machine.
    pub fn start(&mut self, file: Option<String>, addr: Option<String>) {
        let p2p_start;

        self.block_svc.lock().start();

        let chan = self.block_svc.lock().request_channel();
        if is_service_present(&chan) {
            let network_name = self.set_config_from_db();

            let loader = blockchain_loader(chan);
            self.block_svc.lock().wm_arc().lock().set_loader(loader);

            let wm = self.block_svc.lock().wm_arc();
            let db = self.block_svc.lock().db_arc();

            let is_validator = is_validator_function_call(wm, db);

            self.set_block_service_is_validator(is_validator);

            self.p2p_svc.lock().set_network_name(network_name);
            p2p_start = true;
        } else {
            // Load the Boostrap Struct from file
            let (good_network_name, bootstrap_bin, bootstrap_txs) =
                load_bootstrap_struct_from_file(&self.bootstrap_path);

            let wm = self.block_svc.lock().wm_arc();

            let bootstrap_loader = bootstrap_loader(bootstrap_bin);
            self.block_svc
                .lock()
                .wm_arc()
                .lock()
                .set_loader(bootstrap_loader);

            let block_threshold = if bootstrap_txs.is_empty() {
                42
            } else {
                bootstrap_txs.len()
            };

            self.set_block_service_config(BlockchainSettings {
                network_name: Some("bootstrap".to_string()),
                accept_broadcast: false,
                block_threshold,
                block_timeout: 2, // The genesis block will be executed after this timeout and not with block_threshold transactions in the pool // FIXME
            });

            let block_svc = self.block_svc.clone();
            let p2p_svc = self.p2p_svc.clone();

            if bootstrap_txs.is_empty() {
                let wm = self.block_svc.lock().wm_arc();
                let db = self.block_svc.lock().db_arc();

                std::thread::spawn(move || {
                    bootstrap_monitor(chan.clone(), wm.clone());

                    let mut bs = block_svc.lock();
                    let mut config = load_config_from_service(&chan.clone());

                    config.network_name = Some(good_network_name);
                    warn!("network name: {:?}", config.network_name);
                    bs.stop();

                    let net_name = config.network_name.clone().unwrap(); // This shall not fail

                    bs.set_block_config(
                        net_name.clone(),
                        config.block_threshold,
                        config.block_timeout,
                    );

                    // Store the configuration on the DB
                    bs.store_config_into_db(config);

                    let is_validator = is_validator_function_call(wm.clone(), db.clone());
                    bs.set_validator(is_validator);

                    bs.start();
                    p2p_svc.lock().set_network_name(net_name);
                    p2p_svc.lock().start();
                });
                p2p_start = false;
            } else {
                self.put_txs_in_the_pool(bootstrap_txs);

                bootstrap_monitor(chan.clone(), wm); // Blocking function

                let mut config = load_config_from_service(&chan);

                config.network_name = Some(good_network_name);

                // Store the configuration on the DB
                self.store_config_into_db(config);

                let network_name = self.set_config_from_db();

                let wm = self.block_svc.lock().wm_arc();
                let db = self.block_svc.lock().db_arc();

                let is_validator = is_validator_function_call(wm, db);

                self.set_block_service_is_validator(is_validator);

                self.p2p_svc.lock().set_network_name(network_name);
                p2p_start = true;
            }
        }

        warn!("Starting the services");

        self.rest_svc.start();
        if p2p_start {
            self.p2p_svc.lock().start();
        }
        self.bridge_svc.start();

        #[cfg(feature = "monitor")]
        {
            let addr: String = addr.unwrap();
            let file: String = file.unwrap();
            self.monitor_svc.as_mut().unwrap().start(addr, file);
        }
    }

    pub fn park(&mut self) {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let mut stop = false;
            if !self.block_svc.lock().is_running() {
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
            #[cfg(feature = "monitor")]
            {
                if !self.monitor_svc.as_mut().unwrap().is_running() {
                    error!("Monitor service is not running");
                    stop = true;
                }
            }
            if stop {
                self.block_svc.lock().stop();
                self.rest_svc.stop();
                self.p2p_svc.lock().stop();
                self.bridge_svc.stop();
                #[cfg(feature = "monitor")]
                self.monitor_svc.as_mut().unwrap().stop();
                break;
            }
        }
        println!("Something bad happened, stopping the application");
    }
}

#[cfg(test)]
mod tests {
    use crate::app::Bootstrap;
    use glob::glob;
    use trinci_core::{base::serialize::rmp_deserialize, Message};

    #[test]
    fn read_bootstrap_bin() {
        let mut bootstrap_file = std::fs::File::open("./bootstrap.bin").unwrap();
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut bootstrap_file, &mut buf).expect("loading bootstrap");

        let bootstrap = rmp_deserialize::<Bootstrap>(&buf);

        assert!(bootstrap.is_ok());

        println!("{} Transactions", bootstrap.unwrap().txs.len());
    }

    #[test]
    #[ignore = "this is a temporary way to create bootstrap.bin"]
    fn create_bootstrap_bin() {
        let mut bootstrap_file = std::fs::File::open("./service.wasm").unwrap();
        let mut bootstrap = Vec::new();
        std::io::Read::read_to_end(&mut bootstrap_file, &mut bootstrap).expect("loading bootstrap");

        let mut txs = Vec::new();

        for entry in glob("./txs/*.bin").expect("Failed to read glob pattern") {
            match entry {
                Ok(path) => {
                    let filename = path.clone();
                    let filename = filename.file_name().unwrap().to_str().unwrap();

                    if !filename.starts_with("_") {
                        let mut tx_file = std::fs::File::open(path).unwrap();

                        let mut tx_bin = Vec::new();
                        std::io::Read::read_to_end(&mut tx_file, &mut tx_bin)
                            .expect(&format!("Error reading: {:?}", filename));

                        let msg: Message = rmp_deserialize(&tx_bin).unwrap();

                        let tx = match msg {
                            Message::PutTransactionRequest { confirm: _, tx } => tx,
                            _ => panic!("Expected put transaction request message"),
                        };

                        txs.push(tx);
                        println!("Added tx: {}", filename);
                    }
                }
                Err(e) => println!("{:?}", e),
            }
        }

        let bootstrap_bin = Bootstrap {
            bin: bootstrap,
            txs,
        };

        let bootstrap_buf = trinci_core::base::serialize::rmp_serialize(&bootstrap_bin).unwrap();

        std::fs::write("bootstrap.bin", bootstrap_buf).unwrap();
    }
}
