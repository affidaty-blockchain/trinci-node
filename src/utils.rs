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

use std::io::Read;
use trinci_core::{
    crypto::{ecdsa, ed25519, KeyPair},
    Error, ErrorKind, Result,
};

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
                        .unwrap_or(ecdsa::KeyPair::from_pkcs8_bytes(
                            ecdsa::CurveId::Secp384R1,
                            &bytes,
                        )?);
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
