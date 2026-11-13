# Kutu DAO

Kutu DAO is a rotating savings circle MVP for **NEAR Testnet**. A Rust `near-sdk` contract is the sole financial source of truth. The React + TypeScript frontend reads circle, member, round, and accounting views from that contract and submits wallet-signed calls. Supabase is used only for member chat and optional profiles.

## Current status

- The contract implements circle creation, explicit joining, contributions, collateral substitution and cure, permissionless payout finalization, transfer retry, unanimous pre-payout cancellation, refunds, and separate measured storage funding.
- The frontend supports Testnet wallet connection, on-chain reads, contract calls, and member-only circle chat. Its local storage contains only recent circle IDs and browser-local transaction links; its financial state is always read from the contract.
- Supabase stores chat messages and profiles. A signed NEAR challenge authenticates wallet ownership, an Edge Function checks membership against Testnet contract state, and RLS restricts each chat to verified circle members.
- Native Rust and optimized-Wasm local Sandbox results are recorded in [docs/development.md](docs/development.md). The Sandbox checks are opt-in and require Linux or macOS; they do not represent Testnet transactions or a security audit.
- No Testnet contract has been deployed by this project yet. Deployment is a separate manual step; **Mainnet is excluded**.

## Run the frontend

Requirements: Node.js 22.12+ and npm. Install dependencies and start the development server:

```powershell
npm ci
npm run dev
```

To enable contract reads and transactions, copy `frontend/.env.example` to `frontend/.env.local` and set `VITE_CONTRACT_ID` to a contract account on Testnet. The app uses `https://rpc.testnet.near.org` and explicitly selects Testnet for wallet operations. Do not put keys, seed phrases, or other credentials in frontend environment files.

Build the production frontend with `npm run build`.

## Build and test the contract

On Windows, run native unit tests and build optimized Wasm:

```powershell
cargo fmt --manifest-path contract/Cargo.toml -- --check
cargo test --manifest-path contract/Cargo.toml --locked
cargo near build non-reproducible-wasm --locked --no-abi
```

For the opt-in optimized-Wasm Sandbox suite, follow [docs/development.md](docs/development.md). The test uses `near-workspaces` and a local Sandbox network.

## Testnet deployment

Read [docs/testnet-deployment.md](docs/testnet-deployment.md) before contract deployment. It requires a dedicated, user-owned Testnet account, a manual wallet approval, and confirmation that the account has no existing contract state. For chat setup, follow [docs/supabase-chat.md](docs/supabase-chat.md). Never overwrite a deployed account or deploy to Mainnet.

## Project layout

- `contract/` — Rust smart contract and native/Sandbox tests
- `frontend/` — React + TypeScript + Tailwind application
- `docs/` — business rules, architecture, lifecycle, development, and deployment instructions

The MVP uses native NEAR Testnet tokens only. It excludes stablecoins, NEP-141 tokens, Sputnik DAO, Trezu, NEAR Intents for payment, and Mainnet.
