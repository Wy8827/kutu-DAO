# Kutu DAO

Kutu DAO is a rotating savings circle MVP on **NEAR Testnet**. Members join in payout order, contribute once per round, and the contract enforces the circle’s payment and payout rules. The deployed contract is `kutunear.testnet` (version 0.1.0); the deployment transaction reported for this release is [`HDgoZ6pabkwVPSDHXCvyj9PzLTyUff78P68CdXrQMrKA`](https://testnet.nearblocks.io/txns/HDgoZ6pabkwVPSDHXCvyj9PzLTyUff78P68CdXrQMrKA).

## What it does

- Create a circle with a contribution, collateral, target membership, and payment interval.
- Join explicitly; payout order follows join order.
- Contribute, cure a missed payment, and let any account finalize an eligible payout.
- Review on-chain circle, member, round, and accounting state; request contract-defined refund and retry actions.
- Connect a NEAR Testnet wallet through NEAR Connect, which includes Meteor Wallet.
- Optionally use Supabase for member chat and profiles. Supabase does not hold balances or determine financial state.

The Rust contract is the financial source of truth. In local development, contract reads use the Vite same-origin RPC proxy, which uses `NEAR_RPC_URL` from the ignored `frontend/.env.local` when set and otherwise falls back to the public Testnet RPC. The private endpoint is read by the development server only; it is not a `VITE_` variable and is not included in the browser bundle. Production builds currently read from the public Testnet RPC directly. The frontend submits wallet-approved calls to the configured contract. Local storage keeps only recently opened circle IDs and transaction links. The application is Testnet-only; do not use real funds or Mainnet.

The owner reports that Circle #1 was created, joined by two members, completed two contribution rounds, and reached `Completed` on Testnet. This is the reported demo state; this documentation update does not independently query the chain. Circle creation, joining, contributions, and payouts are implemented contract interactions. Supabase chat and profiles are optional; local database tests passed, but hosted Supabase chat delivery has not been confirmed. No formal security audit is claimed.

## Run locally

Requirements: Node.js 22.12+ and npm.

```powershell
npm ci
if (!(Test-Path frontend/.env.local)) { Copy-Item frontend/.env.example frontend/.env.local }
npm run dev
```

The checked-in example uses the deployed Testnet contract ID. Edit `frontend/.env.local` only if needed. To enable optional hosted chat, add the Supabase project URL and public anon key. Never put a service-role key, private key, seed phrase, or wallet credential in Vite variables: `VITE_*` values are public in the browser bundle.

Open the local URL printed by Vite. Select Meteor Wallet (or another supported Testnet wallet), connect, then create a circle or open an existing circle by ID. Every state-changing action requires review and approval in the wallet and spends Testnet NEAR. Connecting the wallet and loading views do not submit transactions.

## Screenshots

The repository does not currently include captured application screenshots. Capture the running app with demo/test data before adding screenshots here; do not present mockups as deployed-contract output.

## Tests and builds

From the repository root:

```powershell
npm test
npm run build
cargo fmt --manifest-path contract/Cargo.toml -- --check
cargo test --manifest-path contract/Cargo.toml --locked
cargo near build non-reproducible-wasm --manifest-path contract/Cargo.toml --locked --no-abi
```

`npm run build` runs the TypeScript check and Vite production build. The optimized Wasm output is `contract/target/near/kutu_dao_contract.wasm`; it is ignored by Git and can be recreated from source. Local NEAR Sandbox integration tests and Supabase database tests are separate opt-in checks described in [docs/development.md](docs/development.md). Passing local tests does not imply a Testnet transaction or hosted Supabase test.

## Configuration

| Variable | Required | Purpose |
| --- | --- | --- |
| `VITE_CONTRACT_ID` | Yes for contract features | NEAR Testnet contract account; currently `kutunear.testnet` |
| `NEAR_RPC_URL` | Optional for local development | Private or custom Testnet RPC endpoint used only by the Vite dev-server proxy; leave blank to use the public RPC. Do not rename with `VITE_`. |
| `VITE_SUPABASE_URL` | Optional | Hosted/local Supabase URL for chat |
| `VITE_SUPABASE_ANON_KEY` | Optional | Supabase public anon/publishable key for chat |

For local development, use `frontend/.env.local` (ignored by Git); copy `frontend/.env.example` as a starting point. `NEAR_RPC_URL` is optional and is only read in Vite development mode. Production build command is `npm run build`, with output directory `frontend/dist`. No Vercel deployment configuration or production server-side RPC proxy is included. Chat additionally requires the Supabase migrations and Edge Functions configured as described in [docs/supabase-chat.md](docs/supabase-chat.md).

## Contract and deployment

The Rust `near-sdk` contract is in [`contract/`](contract/). Its initial deployment and `new()` initialization have already been reported complete on Testnet; this project must not redeploy or reinitialize `kutunear.testnet`. The report and safe read-only verification commands are in [docs/testnet-deployment.md](docs/testnet-deployment.md). Any future upgrade requires a separate reviewed plan that preserves existing state.

The `new()` initializer takes no arguments and initializes an empty circle map. This does not mean the live contract is still empty: users may have created circles after deployment. Read-only view calls do not spend funds. Creating/joining circles, contributions, payouts, and refunds are real Testnet transactions and require wallet approval.

## Project layout

- `contract/` — Rust smart contract, unit tests, and Sandbox integration test
- `frontend/` — React, TypeScript, Vite, and NEAR Connect client
- `supabase/` — database migrations, pgTAP tests, and chat Edge Functions
- `docs/` — business rules, architecture, development, chat, and Testnet guides

See [docs/architecture.md](docs/architecture.md), [docs/development.md](docs/development.md), [docs/business-rules.md](docs/business-rules.md), and [docs/supabase-chat.md](docs/supabase-chat.md) for additional detail. The MVP uses native NEAR only; it does not use USDC, NEP-141, Sputnik DAO, Trezu, or NEAR Intents.
