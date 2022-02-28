# Description

This is a bootstrap which can be used to quickly set up a test TRINCI node. It defines a network in test mode, which means:

- Everyone is validator. Every node running network initialized by this bootstrap can create new blocks without rectrictions.
- Everyone is admin, meaning every account can call methods of the service account reserved to admins only (e.g. `mint`, `burn`, `add_preapproved_contract` etc.).
- Fuel consumption is disabled.
- Network is in test mode, meaning any account (private key) can override smart contract linked to any other account at any moment (except for the service account).
- Everyone can publish a new smart contract ("contract_registration" method f the service account).

## Network Name

QmNiibPaxdU61jSUK35dRwVQYjF9AC3GScWTRzRdFtZ4vZ