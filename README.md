TRINCI
===

TRINCI Blockchain Node.

# 📑 Requirements

The required dependencies to build correctly the project are the following:

- `clang`
- `libclang-dev` _(ver. 11 suggested)_
- `libssl-dev`
- `pkg-config`
- `build-essential`

# 👷 Preparatory Steps

if present remove `Cargo.lock` file:

```bash
$ cd trinci-node/
$ rm Cargo.lock
```
 
## Dependencies installation

### Ubuntu/Debian installation

update the package list:

```bash
$ sudo apt-get update
```

install the dependencies:

```bash
$ sudo apt-get install clang libclang-dev
```

### Fedora/RHEL

update the package list:

```bash
$ sudo dnf check-update
```

install the dependencies:
```bash
$ sudo dnf install clang rust-clang-sys+clang_11_0-devel
```

## 🔨 Build

to build the **node**:

```bash
$ cd ./trinci-node
$ cargo build --release
```

subsequently build the **tools** needed by the `start.sh` script:

```bash
$ cd tools/upnp_negotiator
$ cargo build --release
```

# 🏎️ Node Start-Up

to start the node use the `start.sh` script:

```bash
$ ./start.sh
```

This script make possible to collect the _local IP_
, negotiate a _remote access_ point via uPnP, and use this information for the node monitoring. If you not have access to `ip addr`, `ifconfig`, `hostname`, `gid`, collect those infomrations manually and run the node in this way:

```bash
$ ./trinci --local-ip $local_ip --public-ip $public_ip:$port --rest-port $TARGET_PORT --bootstrap-path $BS_PATH

```

where:

- `local_ip`: is the IP in the local network;

- `public_ip`: is the public IP of the node;

- `port`: is the port where to contact the node;

- `TARGET_PORT`: is node's local port;

- `BS_PATH`: bootstrap path.


# 🧪 Test mode
In order to start the node without kad support (eg for local testing) we can use the flag:

>`-t, --test-mode`    Test mode - the kad network is not started

```
$ cargo run -- --test-mode
```

# ⚠️ Additional Remarks

In case you want to run the node manually, without the help of the `start.sh` script here some suggestions:

## Manual Start-Up
By only running `cargo run`  it launches the node as a follower, this implies that the node can't generate blocks, but only execute those (blocks) present in the p2p network that need to be executed.

## Keypair Generation 
The node only accepts **ECDSA** and **Secp256R1** as keypair loaded from file. If your intention is to use a keypair loaded from file follow this instruction to generate one that respects the requirement.

```bash
$ openssl ecparam -name prime256v1 -genkey -outform DER -out prime256v1_pkcs1.der

$ openssl pkcs8 -topk8 -nocrypt -inform DER -in prime256v1_pkcs1.der -outform DER -out prime256v1_pkcs8_ecdsa.der
```

⚠️ The word ecdsa must be in the key filename: es "myKey_foo_ecdsa.der". In other case the binary throw an error.

## Trinci Boot Phase

In order to start a new node it is necessary the `trinci-node` binary, a `config.toml` (without this the node start with the default values) and a `bootstrap.bin` file.

### `boostrap.bin`

This file is a binary that is a [MessagePack](https://msgpack.org/) of a specific struct:

```json
"Bootstrap":= {
    // Binary of the service contract service.wasm
    "bin": bytes,
    // Vec of transaction for the genesis block
    "txs": [Transaction],
    // Random string to generate unique networkname
    "nonce": string
}
```

 - The first transaction of the `txs` vector needs to be the service account `init` call (this will register the service account).
 - The initial network name for the genesis block is `bootstrap`, then the network name will be the base58 of the `bootstrap.bin` file hash. So each different `bootstrap.bin` creates a different network.
