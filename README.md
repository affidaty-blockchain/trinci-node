TRINCI
======

# Quickstart

# Installation
## Requirements
The required dependencies to build correctly the project are the following:

- clang
- libclang-dev (ver. 11 suggested)
- protobuf-compiler

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
sudo apt-get install clang libclang-dev protobuf-compiler
```

### Fedora/RHEL
update the package list:

```
sudo dnf check-update
```

install the dependencies:
```
sudo dnf install clang rust-clang-sys+clang_11_0-devel protobuf-compiler
```

## Build
to build the cargo package:

```
cd ./trinci-node
cargo build
```


TRINCI Blockchain Node.
