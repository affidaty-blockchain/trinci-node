use std::net::{Ipv4Addr, SocketAddrV4};

pub struct Address {
    pub ip: String,
    pub port: String,
}

pub fn get_port_and_public_ip() -> Address {
    match igd::search_gateway(Default::default()) {
        Err(ref err) => panic!("Error: {}", err),
        Ok(gateway) => {
            let local_addr = match std::env::args().nth(1) {
                Some(local_addr) => local_addr,
                None => panic!("Expected IP address (cargo run -- <your IP here> <port here>)"),
            };

            let port = match std::env::args().nth(2) {
                Some(port) => port,
                None => panic!("Expected port number (cargo run -- <your IP here> <port here>)"),
            };
            let port: u16 = port.parse::<u16>().unwrap();
            let local_addr = local_addr.parse::<Ipv4Addr>().unwrap();
            let local_addr = SocketAddrV4::new(local_addr, port);

            let external_ip = gateway.get_external_ip().unwrap();

            match gateway.add_any_port(
                igd::PortMappingProtocol::TCP,
                local_addr,
                120,
                "node acces point",
            ) {
                Err(ref err) => {
                    panic!("There was an error! {}", err);
                }
                Ok(port) => Address {
                    ip: external_ip.to_string(),
                    port: port.to_string(),
                },
            }
        }
    }
}
