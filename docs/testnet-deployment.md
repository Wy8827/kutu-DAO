# NEAR Testnet deployment record and verification

## Current deployment

The project owner reports that version 0.1.0 was deployed and initialized on NEAR Testnet:

- Contract account: `kutunear.testnet`
- Network: Testnet
- Deployment transaction: [`HDgoZ6pabkwVPSDHXCvyj9PzLTyUff78P68CdXrQMrKA`](https://testnet.nearblocks.io/txns/HDgoZ6pabkwVPSDHXCvyj9PzLTyUff78P68CdXrQMrKA)
- Initializer: `new()` with no JSON arguments (`{}`)

This guide records the supplied deployment details; it does not independently attest the transaction or prove that the currently deployed code matches a local build. **Do not rerun a deploy or initializer command against this account.** This account is already the contract account, and redeployment could replace code while retaining incompatible state. Do not deploy to Mainnet.

The owner reports that Circle #1 was created by `kutunear.testnet`, joined by two members, completed two contribution rounds, and reached `Completed`, which the frontend displayed. This is a reported demonstration result, not a chain query performed while editing this guide. No redeployment is needed for this state. Core circle lifecycle interactions are implemented; hosted Supabase chat remains optional and has not been verified against a hosted project. The project has not undergone a formal security audit.

## Read-only checks

Install/use `near-cli-rs` and explicitly select Testnet for each command. These view operations do not submit transactions:

```powershell
near contract inspect kutunear.testnet network-config testnet now
near contract view-storage kutunear.testnet all as-json network-config testnet now
near contract call-function as-read-only kutunear.testnet get_storage_price_yocto json-args '{}' network-config testnet now
```

To inspect a circle that is known to exist, replace `1` with its actual circle ID:

```powershell
near contract call-function as-read-only kutunear.testnet get_circle json-args '{"circle_id":1}' network-config testnet now
near contract call-function as-read-only kutunear.testnet get_members json-args '{"circle_id":1}' network-config testnet now
```

The initializer creates no circle, so a new deployment would have no valid `get_circle` or `get_members` ID until a circle is created. A missing ID makes these views panic; it does not indicate that the method is absent. The live account may contain circles created since deployment.

## Build and local verification

From the repository root in PowerShell:

```powershell
cargo fmt --manifest-path contract/Cargo.toml -- --check
cargo test --manifest-path contract/Cargo.toml --locked
cargo near build non-reproducible-wasm --manifest-path contract/Cargo.toml --locked --no-abi
Get-FileHash contract/target/near/kutu_dao_contract.wasm -Algorithm SHA256
```

The build target is pinned in `contract/rust-toolchain.toml`; `cargo-near` compiles optimized release Wasm and runs `wasm-opt -O`. The artifact path is `contract/target/near/kutu_dao_contract.wasm`. A previous local artifact was 228,215 bytes with SHA-256 `c05d249dd7dd6e6a2587ad2d0f420424479b1df78ef3cdbb8d5f75630871735b`. Recompute after any source/toolchain change. Local tests and Sandbox execution do not deploy code to Testnet.

## Frontend and chat configuration

The wallet account and contract account are both `kutunear.testnet` for this demonstration. The frontend reads `VITE_CONTRACT_ID` at build time and pins wallet operations and RPC reads to Testnet. Set:

```dotenv
VITE_CONTRACT_ID=kutunear.testnet
```

Optional chat configuration uses `VITE_SUPABASE_URL` and the public `VITE_SUPABASE_ANON_KEY`. The hosted Supabase Edge Functions separately need `NEAR_CONTRACT_ID=kutunear.testnet`, `NEAR_CHAT_AUTH_RECIPIENT` as required by the signed-message implementation, and `CHAT_ALLOWED_ORIGINS` set to the deployed frontend origin(s). Never put private keys, seed phrases, service-role keys, or other server secrets into frontend variables. See [supabase-chat.md](supabase-chat.md) for the full chat setup and remaining hosted-integration checks.

For local frontend development, the Vite proxy reads optional `NEAR_RPC_URL` from ignored `frontend/.env.local`. Leave it unset to use the public Testnet RPC fallback. This setting has no `VITE_` prefix and is used only by the development server, so the private endpoint is not embedded in the client bundle. Production currently reads directly from the public Testnet RPC; no production RPC proxy is configured. `frontend/.env.example` contains an empty placeholder only.

## State-changing operations

Creating a circle, joining, contributing, curing a default, finalizing payouts, cancellation, and refund/retry actions call the existing deployed contract and consume Testnet funds. The browser wallet must display and the user must approve each transaction. Use the frontend for these operations only with disposable demonstration parameters and after reviewing the attached NEAR amount. Do not use CLI deployment examples from older notes or attempt another initialization.
