# Quarantier

Quarantier is an RPC server designed to enhance the reliability of Solana RPC endpoints. It achieves this by monitoring multiple Solana RPCs and temporarily quarantining those that are lagging behind in slot updates. 

## Key Features

- **Optimistic Response Handling**: Quarantier starts by delivering the fastest RPC response to the client.
- **Dynamic Quarantine Management**: Responses from other RPCs are analyzed asynchronously. If an RPC is found to be lagging behind, it is placed in a temporary quarantine.
- **Fast Recovery**: Quarantier allows quarantined RPCs to recover and rejoin the pool of active endpoints once their performance improves.

## How It Works

1. **Initial Response**: When a request is made, Quarantier immediately delivers the fastest available response from the active RPC endpoints.
2. **Response Analysis**: As additional responses come in, Quarantier compares the slots of these responses to detect lagging endpoints.
3. **Quarantine Decisions**: Endpoints that are significantly lagging behind are quarantined to prevent them from impacting overall response quality.
4. **Quarantine Lifecycle**: Quarantined endpoints are periodically re-evaluated and allowed to rejoin once their performance is back to acceptable levels.

## Installation

To install and run Quarantier, ensure you have the necessary dependencies and follow these steps:

1. Clone the repository:
   ```bash
   git clone https://github.com/web3engineering/Quarantier
   cd Quarantier
   ```
2. Install dependencies:
   ```bash
   cargo build --release
   ```
3. Update the port and list of RPCs in `src/main.rs`.
4. Run the server:
   ```bash
   ./target/release/quarantier
   ```

## Usage

Once the server is running, Quarantier acts as a proxy for your Solana RPC requests. Simply point your client to the Quarantier server address, and it will handle the rest.

```bash
curl http://localhost:8080 -d '{"jsonrpc":"2.0","id":1,"method":"getSlot"}'
```

## Limitations

- Quarantine detection and recovery involve some lag due to the optimistic approach.
- Designed primarily for Solana RPCs; customization may be needed for other use cases.

## Contributing

Contributions are welcome!

## License

This project is licensed under the MIT License

---

Quarantier ensures a balance between speed and reliability, making it an essential tool for applications relying on Solana RPC endpoints.
