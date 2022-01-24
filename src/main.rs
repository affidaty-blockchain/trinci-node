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

    let addr: Option<String> = None;
    let file: Option<String> = None;
    #[cfg(feature = "monitor")]
    let (addr, file) = {
        (
            Some(config.monitor_addr.clone()),
            Some(config.monitor_file.clone()),
        )
    };
    let mut app = App::new(config, keypair);
    app.start(file, addr);

    // Temporary blockchain "stuff" tracer.
    let chan = app.block_svc.lock().request_channel();
    std::thread::spawn(move || tracer::run(chan));

    info!("System up and running...");
    app.park();
}
