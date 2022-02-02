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

//! Node configuration
//!
//! Parameters to pragmatically tweak the core behaviour.

use std::{fs, path::Path};
use toml::Value;

/// Default service account.
pub const SERVICE_ACCOUNT_ID: &str = "TRINCI";

/// Default configuration file.
const DEFAULT_CONFIG_FILE: &str = "config.toml";

/// Default logger verbosity level.
pub const DEFAULT_LOG_LEVEL: &str = "info";

/// Default bootstrap file path.
pub const DEFAULT_BOOTSTRAP_PATH: &str = "bootstrap.bin";

/// Default network identifier.
pub const DEFAULT_NETWORK_ID: &str = "bootstrap";

/// Default max transactions per block.
pub const DEFAULT_BLOCK_THRESHOLD: usize = 42;

/// Default block generation max time.
pub const DEFAULT_BLOCK_TIMEOUT: u16 = 3;

/// Default http service binding address.
pub const DEFAULT_HTTP_ADDR: &str = "127.0.0.1";

/// Default http service port.
pub const DEFAULT_HTTP_PORT: u16 = 8000;

/// Default bridge service binding address.
pub const DEFAULT_BRIDGE_ADDR: &str = "127.0.0.1";

/// Default bridge service port.
pub const DEFAULT_BRIDGE_PORT: u16 = 8001;

/// Default p2p service binding address.
pub const DEFAULT_P2P_ADDR: &str = "127.0.0.1";

/// Default p2p service binding port.
pub const DEFAULT_P2P_PORT: u16 = 0;

/// Default database path.
pub const DEFAULT_DB_PATH: &str = "db";

/// Default smart contracts cache size.
pub const DEFAULT_WM_CACHE_MAX: usize = 10;

/// Default monitor file.
pub const DEFAULT_MONITOR_FILE: &str = "blackbox.info";

/// Default monitor addr.
pub const DEFAULT_MONITOR_ADDR: &str =
    "https://dev.exchange.affidaty.net/api/v1/nodesMonitor/update";

/// Core configuration structure.
#[derive(PartialEq, Debug, Clone)]
pub struct Config {
    /// Log level.
    pub log_level: String,
    /// Optional node keypair file.
    pub keypair_path: Option<String>,
    /// Network identifier.
    pub network: String,
    /// Max number of transactions within a block.
    pub block_threshold: usize,
    /// Max number of seconds to trigger block creation if the threshold has not
    /// been reached. Block is created with at least one transaction.
    pub block_timeout: u16,
    /// Http service address.
    pub rest_addr: String,
    /// Http service tcp port.
    pub rest_port: u16,
    /// Bridge service address.
    pub bridge_addr: String,
    /// Bridge service tcp port.
    pub bridge_port: u16,
    /// P2P service ip address.
    pub p2p_addr: String,
    /// P2p service tcp port.
    pub p2p_port: u16,
    /// P2P service bootstrap address.
    pub p2p_bootstrap_addr: Option<String>,
    /// Blockchain database folder path.
    pub db_path: String,
    /// Bootstrap wasm file path.
    pub bootstrap_path: String,
    /// WASM machine max cache size.
    pub wm_cache_max: usize,
    /// Monitor file.
    pub monitor_file: String,
    /// Monitor addr.
    pub monitor_addr: String,
    /// Test mode.
    pub test_mode: bool,
    /// Local IP.
    pub local_ip: Option<String>,
    /// IP seen from the extern.
    pub public_ip: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            log_level: DEFAULT_LOG_LEVEL.to_string(),
            keypair_path: None,
            network: DEFAULT_NETWORK_ID.to_string(),
            block_threshold: DEFAULT_BLOCK_THRESHOLD,
            block_timeout: DEFAULT_BLOCK_TIMEOUT,
            rest_addr: DEFAULT_HTTP_ADDR.to_string(),
            rest_port: DEFAULT_HTTP_PORT,
            bridge_addr: DEFAULT_BRIDGE_ADDR.to_string(),
            bridge_port: DEFAULT_BRIDGE_PORT,
            p2p_addr: DEFAULT_P2P_ADDR.to_string(),
            p2p_port: DEFAULT_P2P_PORT,
            p2p_bootstrap_addr: None,
            db_path: DEFAULT_DB_PATH.to_string(),
            bootstrap_path: DEFAULT_BOOTSTRAP_PATH.to_string(),
            wm_cache_max: DEFAULT_WM_CACHE_MAX,
            monitor_file: DEFAULT_MONITOR_FILE.to_string(),
            monitor_addr: DEFAULT_MONITOR_ADDR.to_string(),
            test_mode: false,
            local_ip: None,
            public_ip: None,
        }
    }
}

impl Config {
    /// Instance a new configuration using options found in the config file.
    /// If a config option is not found in the file, then the default one is used.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Option<Self> {
        let mut config = Self::default();

        let map = match fs::read_to_string(path) {
            Ok(content) => match content.parse::<Value>() {
                Ok(map) => map,
                Err(_err) => {
                    error!("Error: bad config file format");
                    return None;
                }
            },
            Err(_err) => {
                warn!("Warning: config file not found, using default options");
                return Some(config);
            }
        };

        if let Some(value) = map.get("log-level").and_then(|value| value.as_str()) {
            config.log_level = value.to_owned()
        }
        if let Some(value) = map.get("keypair-path").and_then(|value| value.as_str()) {
            config.keypair_path = Some(value.to_owned())
        }
        if let Some(value) = map.get("rest-addr").and_then(|value| value.as_str()) {
            config.rest_addr = value.to_owned();
        }
        if let Some(value) = map.get("rest-port").and_then(|value| value.as_integer()) {
            config.rest_port = value as u16;
        }
        if let Some(value) = map.get("bridge-addr").and_then(|value| value.as_str()) {
            config.bridge_addr = value.to_owned();
        }
        if let Some(value) = map.get("bridge-port").and_then(|value| value.as_integer()) {
            config.bridge_port = value as u16;
        }
        if let Some(value) = map.get("p2p-addr").and_then(|value| value.as_str()) {
            config.p2p_addr = value.to_owned();
        }
        if let Some(value) = map.get("p2p-port").and_then(|value| value.as_integer()) {
            config.p2p_port = value as u16;
        }
        if let Some(value) = map
            .get("p2p-bootstrap-addr")
            .and_then(|value| value.as_str())
        {
            config.p2p_bootstrap_addr = Some(value.to_owned());
        }
        if let Some(value) = map
            .get("block-threshold")
            .and_then(|value| value.as_integer())
        {
            config.block_threshold = value as usize;
        }
        if let Some(value) = map
            .get("block-timeout")
            .and_then(|value| value.as_integer())
        {
            config.block_timeout = value as u16;
        }
        if let Some(value) = map.get("db-path").and_then(|value| value.as_str()) {
            config.db_path = value.to_owned();
        }
        if let Some(value) = map.get("bootstrap-path").and_then(|value| value.as_str()) {
            config.bootstrap_path = value.to_owned();
        }
        if let Some(value) = map.get("wm-cache-max").and_then(|value| value.as_integer()) {
            config.wm_cache_max = value as usize;
        }
        if let Some(value) = map.get("test-mode").and_then(|value| value.as_bool()) {
            config.test_mode = value;
        }
        if let Some(value) = map.get("local-ip").and_then(|value| value.as_str()) {
            config.local_ip = Some(value.to_owned());
        }
        if let Some(value) = map.get("public-ip").and_then(|value| value.as_str()) {
            config.public_ip = Some(value.to_owned());
        }
        Some(config)
    }
}

pub fn create_app_config() -> Config {
    let matches = clap::App::new("T2 Node")
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .about(clap::crate_description!())
        .arg(
            clap::Arg::with_name("config")
                .short("c")
                .long("config")
                .help("Configuration file (default 'config.toml')")
                .value_name("CONFIG")
                .required(false),
        )
        .arg(
            clap::Arg::with_name("log-level")
                .long("log-level")
                .help(&format!("Logger level (default '{}')", DEFAULT_LOG_LEVEL))
                .value_name("LEVEL")
                .required(false)
                .possible_values(&["off", "error", "warn", "info", "debug", "trace"]),
        )
        .arg(
            clap::Arg::with_name("db-path")
                .long("db-path")
                .help(&format!("Database folder (default '{}')", DEFAULT_DB_PATH))
                .value_name("PATH")
                .required(false),
        )
        .arg(
            clap::Arg::with_name("bootstrap-path")
                .long("bootstrap-path")
                .help(&format!(
                    "Bootstrap wasm file path (default '{}')",
                    DEFAULT_BOOTSTRAP_PATH
                ))
                .value_name("PATH")
                .required(false),
        )
        .arg(
            clap::Arg::with_name("http-addr")
                .long("http-addr")
                .help("Http service binding address (default '127.0.0.1')")
                .value_name("ADDRESS")
                .required(false),
        )
        .arg(
            clap::Arg::with_name("http-port")
                .long("http-port")
                .help("Http service listening port (default '8000')")
                .value_name("PORT")
                .required(false),
        )
        .arg(
            clap::Arg::with_name("bridge-addr")
                .long("bridge-addr")
                .help("Bridge service binding address (default '127.0.0.1')")
                .value_name("ADDRESS")
                .required(false),
        )
        .arg(
            clap::Arg::with_name("bridge-port")
                .long("bridge-port")
                .help("Bridge service listening port (default '8001')")
                .value_name("PORT")
                .required(false),
        )
        .arg(
            clap::Arg::with_name("p2p-addr")
                .long("p2p-addr")
                .help("P2P service binding address (default '127.0.0.1')")
                .value_name("ADDRESS")
                .required(false),
        )
        .arg(
            clap::Arg::with_name("p2p-port")
                .long("p2p-port")
                .help("P2P service listening port (default '0')")
                .value_name("PORT")
                .required(false),
        )
        .arg(
            clap::Arg::with_name("p2p-bootstrap-addr")
                .long("p2p-bootstrap-addr")
                .help("peer2peer service bootstrap address (default '127.0.0.1')")
                .value_name("ADDRESS")
                .required(false),
        )
        .arg(
            clap::Arg::with_name("monitor-file")
                .long("monitor-file")
                .help("monitor file location (default 'blackbox.info')")
                .value_name("PATH")
                .required(false),
        )
        .arg(
            clap::Arg::with_name("monitor-addr")
                .long("monitor-address")
                .help("monitor addres to send POST req (default 'https://wowexchange.affidaty.net/api/v1/nodesMonitor/update')")
                .value_name("ADDRESS")
                .required(false),
        )
        .arg(
            clap::Arg::with_name("test-mode")
            .short("t")
            .long("test-mode")
            .help("Test mode - the kad network is not started")
        )
        .arg(
            clap::Arg::with_name("local-ip")
            .long("local-ip")
            .help("Populate the local ip info (default None)")
            .value_name("IP")
            .required(false),
        )
        .arg(
            clap::Arg::with_name("public-ip")
            .long("public-ip")
            .help("Populate the public ip info (default None)")
            .value_name("IP")
            .required(false),
        )
        .get_matches();

    let config_file = matches.value_of("config").unwrap_or(DEFAULT_CONFIG_FILE);
    let mut config = Config::from_file(config_file).expect("Bad config file");

    // Tweak configuration using command line arguments.
    if let Some(value) = matches.value_of("log-level") {
        config.log_level = value.to_owned();
    }
    if let Some(value) = matches.value_of("db-path") {
        config.db_path = value.to_owned();
    }
    if let Some(value) = matches.value_of("boot-path") {
        config.bootstrap_path = value.to_owned();
    }
    if let Some(value) = matches.value_of("http-addr") {
        config.rest_addr = value.to_owned();
    }
    if let Some(value) = matches
        .value_of("http-port")
        .and_then(|value| value.parse::<u16>().ok())
    {
        config.rest_port = value;
    }
    if let Some(value) = matches.value_of("bridge-addr") {
        config.bridge_addr = value.to_owned();
    }
    if let Some(value) = matches
        .value_of("bridge-port")
        .and_then(|value| value.parse::<u16>().ok())
    {
        config.bridge_port = value;
    }
    if let Some(value) = matches.value_of("p2p-addr") {
        config.p2p_addr = value.to_owned();
    }
    if let Some(value) = matches
        .value_of("p2p-port")
        .and_then(|value| value.parse::<u16>().ok())
    {
        config.p2p_port = value;
    }
    if let Some(value) = matches.value_of("p2p-bootstrap-addr") {
        config.p2p_bootstrap_addr = Some(value.to_owned());
    }
    if let Some(value) = matches.value_of("monitor-file") {
        config.monitor_file = value.to_owned();
    }
    if let Some(value) = matches.value_of("monitor-addr") {
        config.monitor_addr = value.to_owned();
    }
    if let Some(value) = matches.value_of("public-ip") {
        config.public_ip = Some(value.to_owned());
    }
    if let Some(value) = matches.value_of("local-ip") {
        config.local_ip = Some(value.to_owned());
    }
    if matches.is_present("test-mode") {
        config.test_mode = true;
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::{self, Display, Formatter};
    use std::io::Write;
    use tempfile::NamedTempFile;

    impl Display for Config {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            write!(
                f,
                "validator = 'FIXME'\n\
                log-level = '{}'\n\
                network = '{}'\n\
                block-threshold = {}\n\
                block-timeout = {}\n\
                rest-addr = '{}'\n\
                rest-port = {}\n\
                bridge-addr = '{}'\n\
                bridge-port = {}\n\
                p2p-addr = '{}'\n\
                p2p-port = '{}'\n\
                p2p-bootstrap-addr = '{}'\n\
                db-path = '{}'\n\
                bootstrap-path = '{}'\n\
                wm-cache-max = {}",
                self.log_level,
                self.network,
                self.block_threshold,
                self.block_timeout,
                self.rest_addr,
                self.rest_port,
                self.bridge_addr,
                self.bridge_port,
                self.p2p_addr,
                self.p2p_port,
                self.p2p_bootstrap_addr.clone().unwrap_or_default(),
                self.db_path,
                self.bootstrap_path,
                self.wm_cache_max
            )
        }
    }

    fn create_test_config() -> Config {
        Config {
            log_level: "debug".to_string(),
            keypair_path: None,
            network: "bootstrap".to_string(),
            block_threshold: 1234,
            block_timeout: 4321,
            rest_addr: "1.2.3.4".to_string(),
            rest_port: 123,
            bridge_addr: "5.6.7.8".to_string(),
            bridge_port: 987,
            p2p_addr: "9.1.2.3".to_string(),
            p2p_port: 0,
            p2p_bootstrap_addr: Some("1.0.0.3".to_string()),
            db_path: "dummy/db/path".to_string(),
            bootstrap_path: "dummy/boot/path".to_string(),
            wm_cache_max: 42,
            monitor_file: "blackbox.info".to_string(),
            monitor_addr: "https://dev.exchange.affidaty.net/api/v1/nodesMonitor/update"
                .to_string(),
            test_mode: false,
            local_ip: None,
            public_ip: None,
        }
    }

    #[test]
    fn from_file() {
        let default_config = create_test_config();
        let mut file = NamedTempFile::new().unwrap();
        let _ = writeln!(&mut file, "{}", default_config);
        let filename = file.path().as_os_str().to_string_lossy().to_string();

        let config = Config::from_file(filename).unwrap();

        assert_eq!(config, default_config);
    }
}
