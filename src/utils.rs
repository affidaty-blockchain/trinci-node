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
    crypto::{ed25519, KeyPair},
    Error, ErrorKind, Result,
};

/// Load node account keypair.
pub fn load_ed25519_keypair(filename: &str) -> Result<KeyPair> {
    info!("Loading node keys from: {}", filename);
    let mut file = std::fs::File::open(filename)
        .map_err(|err| Error::new_ext(ErrorKind::MalformedData, err))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).expect("loading node keypair");

    let keypair = ed25519::KeyPair::from_bytes(&bytes)?;
    Ok(KeyPair::Ed25519(keypair))
}
