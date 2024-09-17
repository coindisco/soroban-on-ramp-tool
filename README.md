# Soroban Swap Tool

This project implements a swap pool contract using Rust and the Soroban SDK. The tool allows users to buy any stellar asset with classic on ramp providers leveraging soroban smart contracts.

## Features

- **User Management**: Easily manage user roles and permissions.
- **Wallet Integration**: Seamlessly add and manage wallets for different tokens.
- **Swap Transactions**: Effortlessly process swap transactions.
- **Fee Management**: Set and manage transaction fees.
- **Contract Upgrades**: Support for upgrading the contract without downtime.

## Prerequisites

- Rust
- Cargo
- Soroban SDK
- Taskfile

## Getting Started

### Installation

1. Clone the repository:
   ```
   git clone git@github.com:coindisco/soroban-on-ramp-tool.git  
   cd soroban-on-ramp-tool
   ```

2. Build the project:  
   `task build`

### Running Tests

To run the tests, use the following command:  
`task test`

## Usage

### Deploying the Contract

To deploy the contract, use the following command:  
`soroban contract deploy --wasm <path_to_wasm_file> --network <network> --secret-key <secret_key>`

### Interacting with the Contract

You can interact with the contract using the provided methods in `PoolContractInterface` and `UpgradeableContract`.

## Project Structure

- `contracts/pool/src/contract.rs`: Main contract implementation.
- `contracts/pool/src/test.rs`: Test cases for the contract.
- `contracts/pool/src/errors.rs`: Error definitions.
- `contracts/pool/src/interfaces.rs`: Interface definitions.
- `contracts/pool/src/storage.rs`: Storage management functions.
- `contracts/pool/src/swap_router.rs`: Swap router logic.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request for any changes.
