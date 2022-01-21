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

#[macro_use]
extern crate log;

mod app;
mod config;
mod tracer;
mod utils;

#[cfg(feature = "monitor")]
mod monitor;

use crate::app::App;
use config::Config;
use log::LevelFilter;
use simplelog::{ColorChoice, TermLogger, TerminalMode};
use std::env;

/// Logger initialization.
/// Output is set to standard output.
fn logger_init() {
    let config = simplelog::ConfigBuilder::new()
        .add_filter_allow_str("trinci")
        .build();

    TermLogger::init(
        LevelFilter::Trace,
        config,
        TerminalMode::Stdout,
        ColorChoice::Auto,
    )
    .expect("logger init");
}

/// Sets logger verbosity level.
fn logger_level(level: &str) {
    let level = match level {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Debug,
    };
    log::set_max_level(level);
}

/// Prints the node configuration.
fn show_config(config: &Config) {
    let keypair_path = config.keypair_path.as_deref().unwrap_or("null");
    info!("Configuration:");
    info!("  Validator:            //  FIXME");
    info!("  Keypair path:           {}", keypair_path);
    info!("  Network Id:             {}", config.network);
    info!("  Block threshold:        {}", config.block_threshold);
    info!("  Block timeout:          {}", config.block_timeout);
    info!("  Database path:          {}", config.db_path);
    info!("  Boot files path:        {}", config.bootstrap_path);
    info!("  WM cache max size:      {}", config.wm_cache_max);
    info!(
        "  REST service address:   {}:{}",
        config.rest_addr, config.rest_port
    );
    info!(
        "  Bridge service address: {}:{}",
        config.bridge_addr, config.bridge_port
    );
    info!("  P2P service address:    {}", config.p2p_addr);
    info!(
        "  P2P bootstrap address:  {}",
        config.p2p_bootstrap_addr.clone().unwrap_or_default()
    );
    if config.test_mode {
        info!("  Test mode:  Active");
    }
}

fn main() {
    logger_init();
    let config = config::create_app_config();
    logger_level(&config.log_level);

    info!("Starting TRINCI Node");
    info!("  Node version:         {}", env!("CARGO_PKG_VERSION"));
    info!("  Core version:         {}", trinci_core::VERSION);

    show_config(&config);

    let filename = config.keypair_path.clone();
    let keypair = utils::load_keypair(filename).expect("keypair generation fail");
    info!("Node ID: {}", keypair.public_key().to_account_id());

    #[cfg(feature = "monitor")]
    let (node_id, public_key) = {
        (
            keypair.public_key().to_account_id(),
            match keypair.public_key() {
                trinci_core::PublicKey::Ecdsa(key) => key.to_account_id(),
                trinci_core::PublicKey::Ed25519 { pb } => pb.to_account_id(),
            },
        )
    };

    let mut app = App::new(config, keypair);
    app.start();

    // Temporary blockchain "stuff" tracer.
    let chan = app.block_svc.lock().request_channel();
    std::thread::spawn(move || tracer::run(chan));

    // block chain monitor
    #[cfg(feature = "monitor")]
    {
        debug!("[monitor] monitor started");
        let config = config::create_app_config();

        let nw_public_key = app.p2p_public_key.to_account_id();
        let public_ip = monitor::get_ip();

        let node_status = monitor::Status {
            public_key,
            nw_public_key,
            ip_endpoint: None,
            role: monitor::NodeRole::Ordinary, // FIXME
            nw_config: monitor::NetworkConfig {
                name: config.network,
                //name: todo!(),
                block_threshold: config.block_threshold,
                block_timeout: config.block_timeout,
            },
            core_version: trinci_core::VERSION.to_string(),
            last_block: None,
            unconfirmed_pool: None,
            p2p_info: monitor::P2pInfo {
                p2p_addr: config.p2p_addr,
                p2p_port: config.p2p_port,
                p2p_bootstrap_addr: config.p2p_bootstrap_addr,
            },
            pub_ip: public_ip,
        };

        let mut monitor_struct = monitor::Monitor::new(node_id, node_status);
        let chan = app.block_svc.lock().request_channel();
        let addr = config.monitor_addr.clone();
        let file = config.monitor_file;

        std::thread::spawn(move || monitor::run(&mut monitor_struct, chan, &addr, &file));
    }

    info!("System up and running...");
    app.park();
}
