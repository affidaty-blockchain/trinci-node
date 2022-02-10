use upnp_negotiator::get_port_and_public_ip;

fn main() {
    let info = get_port_and_public_ip();
    println!("{}:{}", info.ip, info.port);
}
