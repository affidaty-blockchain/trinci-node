ChangeLog
=========

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com).

Type of changes

* Added: for new features.
* Changed: for changes in existing functionality.
* Deprecated: for soon-to-be removed features.
* Removed: for now removed features.
* Fixed: for any bug fixes.
* Security: in case of vulnerabilities.

This project adheres to [Semantic Versioning](http://semver.org).

Given a version number MAJOR.MINOR.PATCH
* MAJOR incremented for incompatible API changes
* MINOR incremented for new functionalities
* PATCH incremented for bug fixes

Additional labels for pre-release metadata:
* alpha.x: internal development stage.
* beta.x: shipped version under testing.
* rc.x: stable release candidate.

0.2.9-rc1 28-07-2022
----------------
Added
 * Indexer feature
 * Now node can be contacted via rest to recover bootstrap and network informations.

0.2.8 05-07-2022
----------------
Changed
* Bulk transaction is paid by the signer

Fixed
* Block time fixed (before was always zero) ### Breaking Change


0.2.7 - 24-05-2022
----------------------
* Call to Service `contract_updatable`
* NFA Non-Fungible Account
* Fixed Drand method
* Monitor does not send data if offline mode is active

0.2.7-rc1 - 16-02-2022
------------------
Changed
* BlockchainSettings structure
* `test-mode` flag renamed in `offline`

0.2.6 - 08-02-2022
------------------

Changed
* removed wasm loader from closure

0.2.5 - 02-02-2022
------------------

Added
* test mode to p2p module (prevent it from start)
* ip address from command line

Changed
* new monitor architecture
