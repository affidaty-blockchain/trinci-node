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

#[cfg(feature = "monitor")]
use crate::monitor::{self, service::MonitorService, worker::MonitorConfig};
use crate::utils;
use crate::{config::Config, config::SERVICE_ACCOUNT_ID};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use trinci_core::base::BlockchainSettings;
use trinci_core::crypto::drand::SeedSource;
use trinci_core::crypto::{Hash, HashAlgorithm};
use trinci_core::db::DbFork;

use trinci_core::{wm::local::MAX_FUEL, Account, Error, VERSION};

use version_compare::Cmp;

use trinci_core::{
    base::{
        serialize::{rmp_deserialize, rmp_serialize},
        Mutex, RwLock,
    },
    blockchain::{BlockConfig, BlockRequestSender, BlockService, Event, Message},
    bridge::{BridgeConfig, BridgeService},
    crypto::{ed25519::KeyPair as Ed25519KeyPair, ed25519::PublicKey as Ed25519PublicKey, KeyPair},
    db::{Db, RocksDb, RocksDbFork},
    p2p::{service::PeerConfig, PeerService},
    rest::{RestConfig, RestService},
    wm::{Wm, WmLocal},
    ErrorKind, Transaction,
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
    #[cfg(feature = "monitor")]
    pub monitor_svc: Option<MonitorService>,
    /// Keypair placeholder.
    pub keypair: Arc<KeyPair>,
    /// p2p Keypair placeholder
    pub p2p_public_key: Ed25519PublicKey,
    /// Bootstrap path
    pub bootstrap_path: String,
    /// Seed
    pub seed: Arc<SeedSource>,
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
    seed: Arc<SeedSource>,
) -> impl IsValidator {
    move |account_id: String| {
        let args = rmp_serialize(&account_id)?;

        let seed = seed.clone();
        let mut fork = db.write().fork_create();
        let mut events = Vec::new();

        let account = fork
            .load_account(SERVICE_ACCOUNT_ID)
            .ok_or_else(|| Error::new_ext(ErrorKind::Other, "The Service Account must exist!"))?;

        let contract = account.contract.ok_or_else(|| {
            Error::new_ext(
                ErrorKind::Other,
                "The Service Account must have a contract!",
            )
        })?;
        let (_, res) = wm.lock().call(
            &mut fork,
            42,
            "skynet",
            SERVICE_ACCOUNT_ID,
            SERVICE_ACCOUNT_ID,
            SERVICE_ACCOUNT_ID,
            contract,
            "is_validator",
            &args,
            seed,
            &mut events,
            MAX_FUEL,
        );
        let res = res?;

        rmp_deserialize(&res)
    }
}

fn bootstrap_monitor(chan: BlockRequestSender) {
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
                    debug!("Bootstrap is over, switching to a better validator check...");

                    break;
                } else {
                    panic!("Block constructed but 'service' account is not yet active");
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

// Load the bootstrap struct from file, panic if something goes wrong
fn load_bootstrap_struct_from_file(path: &str) -> (String, Vec<u8>, Vec<Transaction>) {
    let mut bootstrap_file = std::fs::File::open(path).expect("bootstrap file not found");

    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut bootstrap_file, &mut buf).expect("loading bootstrap");

    match rmp_deserialize::<Bootstrap>(&buf) {
        Ok(bs) => (calculate_network_name(&buf), bs.bin, bs.txs),
        Err(_) => panic!("Invalid bootstrap file format!"), // If the bootstrap is not valid should panic!
    }
}
#[derive(Serialize, Deserialize)]
struct Bootstrap {
    // Binary bootstrap.wasm
    #[serde(with = "serde_bytes")]
    bin: Vec<u8>,
    // Vec of transaction for the genesis block
    txs: Vec<Transaction>,
    // Random string to generate unique file
    nonce: String,
}

// If this panics, it panics early at node boot. Not a big deal.
// This should be called only once after the genesis block
pub(crate) fn load_config_from_service(chan: &BlockRequestSender) -> BlockchainSettings {
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
        let wm = WmLocal::new(config.wm_cache_max);
        let db = RocksDb::new(&config.db_path);

        let keypair = Arc::new(keypair);

        let block_config = BlockConfig {
            threshold: config.block_threshold,
            timeout: config.block_timeout,
            network: config.network.clone(),
            keypair: keypair.clone(),
        };

        let is_validator = is_validator_function_temporary(true);

        // seed initialization
        let nonce: Vec<u8> = vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        let prev_hash =
            Hash::from_hex("1220d4ff2e94b9ba93c2bd4f5e383eeb5c5022fd4a223285629cfe2c86ed4886f730")
                .unwrap();
        let txs_hash =
            Hash::from_hex("1220d4ff2e94b9ba93c2bd4f5e383eeb5c5022fd4a223285629cfe2c86ed4886f730")
                .unwrap();
        let rxs_hash =
            Hash::from_hex("1220d4ff2e94b9ba93c2bd4f5e383eeb5c5022fd4a223285629cfe2c86ed4886f730")
                .unwrap();
        let seed = SeedSource::new(config.network.clone(), nonce, prev_hash, txs_hash, rxs_hash);
        let seed = Arc::new(seed);
        #[cfg(feature = "monitor")]
        let seed_value = seed.get_seed();

        // Needed in p2p service and blockchain information gathering
        let (p2p_public_key, p2p_keypair) = if config.p2p_keypair.is_some() {
            let p2p_keypair = utils::load_keypair(config.p2p_keypair).unwrap();
            let p2p_keypair = match p2p_keypair {
                KeyPair::Ecdsa(_) => panic!("P2P keypair should be ED25519"),
                KeyPair::Ed25519(kp) => kp,
            };
            debug!("[p2p] keypair loaded from file");
            (p2p_keypair.public_key(), p2p_keypair)
        } else {
            let p2p_keypair = Ed25519KeyPair::from_random();
            debug!("[p2p] keypair randomly generated");
            (p2p_keypair.public_key(), p2p_keypair)
        };

        let block_svc = BlockService::new(
            &keypair.public_key().to_account_id(),
            is_validator,
            block_config,
            db,
            wm,
            seed.clone(),
            p2p_public_key.to_account_id(),
        );
        let chan = block_svc.request_channel();

        let rest_config = RestConfig {
            addr: config.rest_addr.clone(),
            port: config.rest_port,
        };
        let rest_svc = RestService::new(rest_config, chan.clone());

        let p2p_config = PeerConfig {
            addr: config.p2p_addr.clone(),
            port: config.p2p_port,
            network: Mutex::new(config.network.clone()),
            bootstrap_addr: config.p2p_bootstrap_addr.clone(),
            p2p_keypair: Some(p2p_keypair),
            active: !config.offline,
        };
        let p2p_svc = PeerService::new(p2p_config, chan.clone());

        let bridge_config = BridgeConfig {
            addr: config.bridge_addr,
            port: config.bridge_port,
        };
        let bridge_svc = BridgeService::new(bridge_config, chan.clone());

        // block chain monitor
        #[cfg(feature = "monitor")]
        let monitor_svc = {
            let nw_public_key = p2p_public_key.to_account_id();

            let node_status = monitor::worker::Status {
                public_key: keypair.public_key().to_account_id(), // check if ok
                nw_public_key,
                role: monitor::worker::NodeRole::Ordinary, // FIXME
                nw_config: monitor::worker::NetworkConfig {
                    name: config.network,
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
                ip_endpoint: config.local_ip,
                pub_ip: config.public_ip,
                seed: seed_value,
            };

            let monitor_config = MonitorConfig {
                nodeID: keypair.public_key().to_account_id(),
                data: node_status,
            };

            MonitorService::new(monitor_config, chan, config.offline)
        };

        App {
            block_svc: Arc::new(Mutex::new(block_svc)),
            rest_svc,
            p2p_svc: Arc::new(Mutex::new(p2p_svc)),
            bridge_svc,
            p2p_public_key,
            bootstrap_path: config.bootstrap_path,
            keypair,
            #[cfg(feature = "monitor")]
            monitor_svc: Some(monitor_svc),
            seed,
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
        self.block_svc
            .lock()
            .set_burn_fuel_method(config.burning_fuel_method);
        self.block_svc.lock().start();
    }

    // Load the config from the DB
    fn set_config_from_db(&mut self) -> String {
        let block_svc = self.block_svc.clone();
        let db = block_svc.lock().db_arc();
        let buf = db.read().load_configuration("blockchain:settings").unwrap(); // If this fails is at the very beginning
        let config = rmp_deserialize::<BlockchainSettings>(&buf).unwrap(); // If this fails is at the very beginning

        // Check core version
        let version = VERSION;
        match version_compare::compare(version, config.min_node_version.clone()) {
            Ok(Cmp::Lt) => {
                panic!(
                    "Error: The core version is lower than the minumum accepted by the bootstrap"
                )
            }
            Ok(_) => (),
            Err(_) => panic!("Error: Version comparing failure"),
        }

        let network_name = config.network_name.clone().unwrap(); // If this fails is at the very beginning
        info!("network name: {:?}", network_name);
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

    // Store manually the service Account on the DB
    fn store_service_account(
        &self,
        db: Arc<RwLock<dyn Db<DbForkType = RocksDbFork>>>,
        bootstrap_bin: Vec<u8>,
    ) {
        let mut fork = db.write().fork_create();
        let hash = Hash::from_data(HashAlgorithm::Sha256, &bootstrap_bin);
        fork.store_account(Account::new(SERVICE_ACCOUNT_ID, Some(hash)));
        let mut key = String::from("contracts:code:");
        key.push_str(&hex::encode(&hash));
        fork.store_account_data(SERVICE_ACCOUNT_ID, &key, bootstrap_bin);
        db.write().fork_merge(fork).unwrap();
    }

    /// Starts the blockchain service to receive messages from the bootstrap procedure.
    /// Spawn a temporary thread that takes care of "service" account creation.
    /// Once that the service account is created, the thread takes care to set the
    /// main smart contracts loader within the wasm machine.
    pub fn start(&mut self, _file: Option<String>, _addr: Option<String>) {
        let p2p_start;

        self.block_svc.lock().start();

        let db = self.block_svc.lock().db_arc();

        let chan = self.block_svc.lock().request_channel();
        if is_service_present(&chan) {
            let network_name = self.set_config_from_db();

            let wm = self.block_svc.lock().wm_arc();

            let is_validator = is_validator_function_call(wm, db, self.seed.clone());

            self.set_block_service_is_validator(is_validator);

            self.p2p_svc.lock().set_network_name(network_name);
            p2p_start = true;
        } else {
            // Load the Bootstrap Struct from file
            let (good_network_name, bootstrap_bin, bootstrap_txs) =
                load_bootstrap_struct_from_file(&self.bootstrap_path);

            // Store the service account on the DB
            self.store_service_account(db, bootstrap_bin);

            let block_threshold = if bootstrap_txs.is_empty() {
                42
            } else {
                bootstrap_txs.len()
            };

            self.set_block_service_config(BlockchainSettings {
                accept_broadcast: false,
                block_threshold,
                block_timeout: 2, // The genesis block will be executed after this timeout and not with block_threshold transactions in the pool // FIXME
                burning_fuel_method: String::new(),
                network_name: Some("bootstrap".to_string()),
                is_production: true,
                min_node_version: String::from("0.2.6"),
            });

            let block_svc = self.block_svc.clone();
            let p2p_svc = self.p2p_svc.clone();

            if bootstrap_txs.is_empty() {
                let wm = self.block_svc.lock().wm_arc();
                let db = self.block_svc.lock().db_arc();
                let seed = self.seed.clone();

                std::thread::spawn(move || {
                    bootstrap_monitor(chan.clone());

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

                    // Set the burn fuel method name
                    bs.set_burn_fuel_method(config.burning_fuel_method.clone());

                    // Store the configuration on the DB
                    bs.store_config_into_db(config);

                    let is_validator = is_validator_function_call(wm.clone(), db.clone(), seed);
                    bs.set_validator(is_validator);

                    bs.start();
                    p2p_svc.lock().set_network_name(net_name);
                    p2p_svc.lock().start();
                });
                p2p_start = false;
            } else {
                self.put_txs_in_the_pool(bootstrap_txs);

                bootstrap_monitor(chan.clone()); // Blocking function

                let mut config = load_config_from_service(&chan);

                config.network_name = Some(good_network_name);

                // Store the configuration on the DB
                self.store_config_into_db(config);

                let network_name = self.set_config_from_db();

                let wm = self.block_svc.lock().wm_arc();
                let db = self.block_svc.lock().db_arc();

                let is_validator = is_validator_function_call(wm, db, self.seed.clone());

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
            let addr: String = _addr.unwrap();
            let file: String = _file.unwrap();
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
    use trinci_core::base::serialize::rmp_deserialize;

    #[ignore = "use this to check a boostrap file"]
    #[test]
    fn read_bootstrap_bin() {
        let mut bootstrap_file = std::fs::File::open("./bootstrap.bin").unwrap();
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut bootstrap_file, &mut buf).expect("loading bootstrap");

        let bootstrap = rmp_deserialize::<Bootstrap>(&buf);

        assert!(bootstrap.is_ok());
        let bootstrap = bootstrap.unwrap();
        println!("{} Transactions", &bootstrap.txs.len());
        println!("nonce: `{}`", &bootstrap.nonce);
    }
}
