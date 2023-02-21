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

use isahc::ReadResponseExt;
use std::{
    fs::File,
    io::{Read, Write},
};
use trinci_core::{
    crypto::{ecdsa, ed25519, KeyPair},
    rest::service::NodeInfo,
    Error, ErrorKind, Result,
};

use ring::digest;

/// Load node account keypair.
pub fn load_keypair(filename: Option<String>) -> Result<KeyPair> {
    match filename {
        Some(filename) => {
            info!("Loading node keys from: {}", filename);
            if filename.contains("/tpm") {
                #[cfg(not(feature = "tpm2"))]
                panic!(
                    "TPM2 feature not included, for using tpm2 module compile with feature=tpm2"
                );
                #[cfg(feature = "tpm2")]
                {
                    let ecdsa =
                        ecdsa::KeyPair::new_tpm2(ecdsa::CurveId::Secp256R1, filename.as_str())?;
                    Ok(KeyPair::Ecdsa(ecdsa))
                }
            } else {
                let mut file = std::fs::File::open(&filename)
                    .map_err(|err| Error::new_ext(ErrorKind::MalformedData, err))?;
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).expect("loading node keypair");
                if filename.contains("ecdsa") {
                    let ecdsa = ecdsa::KeyPair::from_pkcs8_bytes(ecdsa::CurveId::Secp256R1, &bytes)
                        .or_else(|_| {
                            ecdsa::KeyPair::from_pkcs8_bytes(ecdsa::CurveId::Secp384R1, &bytes)
                        })?;
                    Ok(KeyPair::Ecdsa(ecdsa))
                } else {
                    let ed25519 = ed25519::KeyPair::from_bytes(&bytes)?;
                    Ok(KeyPair::Ed25519(ed25519))
                }
            }
        }
        None => {
            let ed25519 = ed25519::KeyPair::from_random();
            Ok(KeyPair::Ed25519(ed25519))
        }
    }
}

/// Collects node visa.
pub fn get_visa(node_address: &str) -> Result<NodeInfo> {
    match isahc::get(format!("{}/api/v1/visa", node_address)) {
        Ok(mut response) => Ok(response.json().unwrap()),
        Err(_) => Err(Error::new(ErrorKind::Other)),
    }
}

/// Collects bootstrap file.
pub fn get_bootstrap(node_address: &str, bootstrap_path: String) -> String {
    match isahc::get(format!("{}/api/v1/bootstrap", node_address)) {
        Ok(mut response) => {
            println!("bootstrap retrieved");

            let bootstrap_bytes = response.bytes().unwrap();

            let mut hash = digest::digest(&digest::SHA256, &bootstrap_bytes)
                .as_ref()
                .to_vec();

            let mut pre_hash: Vec<u8> = [0x12, 0x20].to_vec();
            pre_hash.append(&mut hash);

            let bs58 = bs58::encode(pre_hash);
            let bootstrap_hash = bs58.into_string();
            let bootstrap_path = format!("data/{}.bin", bootstrap_hash);

            let mut file = File::create(&bootstrap_path).unwrap();
            file.write(&bootstrap_bytes).unwrap();
            bootstrap_hash
        }
        Err(error) => {
            println!("Error occourred during get request: {}", error.to_string());
            bootstrap_path
        }
    }
}

/// Given local and remote node version comunicates
/// to the user if the local verison conflicts with the remote one.
pub fn check_version(local_version: (String, String), remote_version: (String, String)) {
    match (
        version_compare::compare(local_version.0, remote_version.0).unwrap(),
        version_compare::compare(remote_version.1, local_version.1).unwrap(),
    ) {
        (version_compare::Cmp::Lt, _) => warn!("local node version not up to date"),

        (version_compare::Cmp::Gt, _) => {
            warn!("local node version more recent than bootstrap node verison")
        }
        (_, version_compare::Cmp::Lt) => warn!("local core version not up to date"),
        (_, version_compare::Cmp::Gt) => {
            warn!("local core version more recent than bootstrap node verison")
        }
        (_, _) => (),
    }
}
