TRINCI
======

TRINCI Blockchain Node.

## Requirements

The required dependencies to build correctly the project are the following:

- clang
- libclang-dev (ver. 11 suggested)

follow the installations for the most common Unix/Linux systems 

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

To make possible to the node to generate new blocks, run it as validator:

```
cargo run -- --validator
```

**Note:** by only running `cargo run`  it launches the node as a follower, this implies that the node can't generate blocks, but only execute those (blocks) present in the p2p network that need to be executed.
