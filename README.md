TRINCI
======

TRINCI Blockchain Node.

## Requirements

The required dependencies to build correctly the project are the following:

- clang
- libclang-dev (ver. 11 suggested)
- libssl-dev
- pkg-config
- build-essential

## preparatory steps

remove `Cargo.lock file`

```
cd trinci-node/
rm Cargo.lock
```
 
## Dependencies installation

### Ubuntu/Debian installation

update the package list:

```
sudo apt-get update
```

install the dependencies:

```
sudo apt-get install clang libclang-dev
```

### Fedora/RHEL

update the package list:

```
sudo dnf check-update
```

install the dependencies:
```
sudo dnf install clang rust-clang-sys+clang_11_0-devel
```

## Build

to build the cargo package:

```
cd ./trinci-node
cargo build
```

# Initialization

**Note:** by only running `cargo run`  it launches the node as a follower, this implies that the node can't generate blocks, but only execute those (blocks) present in the p2p network that need to be executed.

# Keypair generation

The node only accepts **ECDSA Secp256R1** as keypair loaded from file. If your intention is to use a keypair loaded from file follow this instruction to generate one that respects the requirement.

```
openssl ecparam -name prime256v1 -genkey -outform DER -out prime256v1_pkcs1.der

openssl pkcs8 -topk8 -nocrypt -inform DER -in prime256v1_pkcs1.der -outform DER -out prime256v1_pkcs8.der
```
