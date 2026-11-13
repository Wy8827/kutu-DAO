# NEAR Testnet deployment guide

This guide describes a future manual Testnet deployment. **No deployment has been performed. Do not use Mainnet.** Deployment requires the account owner's wallet authorization and an explicit review of the account before sending the deploy transaction.

## Safety requirements

- Use a dedicated account ending in `.testnet` that you control and can authorize from a NEAR Testnet wallet.
- Do not share, paste, or commit a seed phrase, private key, key file, or wallet credentials. `near-cli-rs` can use its interactive Testnet keychain flow.
- Before deploying, inspect the target account on Testnet and verify its owner/access keys and contract state. The account must be intended for this project and have no existing contract code or state. If it has ever been used for another contract or prototype, stop and choose a fresh account; this contract has no migration method.
- The deploy command replaces contract code. Never deploy over an account whose state or ownership you have not verified.
- This project has no Mainnet deployment path or authorization.

## 1. Build and record the Wasm

From the repository root in PowerShell:

```powershell
cargo fmt --manifest-path contract/Cargo.toml -- --check
cargo test --manifest-path contract/Cargo.toml --locked
cargo near build non-reproducible-wasm --locked --no-abi
Get-FileHash contract/target/near/kutu_dao_contract.wasm -Algorithm SHA256
```

The Windows build verified for this handover was 228,215 bytes with SHA-256 `c05d249dd7dd6e6a2587ad2d0f420424479b1df78ef3cdbb8d5f75630871735b`. Rebuild and recalculate the hash for any source or toolchain change; deploy only the artifact whose hash you reviewed.

Record the output hash alongside the release notes and verify it again before deployment. This command builds optimized Wasm but is labeled non-reproducible by `cargo-near`; matching hashes should be confirmed by rebuilding in the intended release environment. The Wasm is a build artifact and is ignored by Git; rebuild it from the recorded source revision. The current Windows build skips ABI generation with `--no-abi` because the ABI generator cannot link NEAR host functions in the Windows environment.

For the optimized-Wasm local Sandbox checks, follow the WSL2 instructions in [development.md](development.md). Sandbox is a local test network, not Testnet.

## 2. Inspect and deploy to a fresh Testnet account

Install `near-cli-rs` and authenticate the account owner through the CLI's interactive Testnet keychain flow. Replace `<your-account>.testnet` below with the candidate account ID, then inspect it using Testnet-only CLI commands:

```powershell
near account view-account-summary <your-account>.testnet network-config testnet now
near contract inspect <your-account>.testnet network-config testnet now
near contract view-storage <your-account>.testnet all as-json network-config testnet now
```

Confirm the account ID and access keys are yours, the account has no deployed contract code, and it has no contract state. If inspection is unclear, the account has any existing contract/state, or you cannot verify ownership, stop and use a freshly created account instead.

After confirming the account is fresh and you are ready to authorize deployment, replace `<your-account>.testnet` below with that verified account ID. Run from the repository root:

```powershell
near contract deploy <your-account>.testnet `
  use-file contract/target/near/kutu_dao_contract.wasm `
  with-init-call new `
  json-args '{}' `
  prepaid-gas '100 TGas' `
  attached-deposit '0 NEAR' `
  network-config testnet `
  sign-with-keychain `
  send
```

Review the CLI transaction details and confirm in your wallet. Do not use `sign-with-plaintext-private-key` or put credentials in command arguments or environment variables. Keep the transaction result and deployed account ID with the release record. A successful local build is not evidence of a successful Testnet deployment.

## 3. Configure and verify the frontend

Copy `frontend/.env.example` to `frontend/.env.local`, then set `VITE_CONTRACT_ID` to the verified deployed `.testnet` account. The RPC URL is fixed to `https://rpc.testnet.near.org`; wallet connection, account retrieval, signing, signing out, and transaction submission explicitly use Testnet.

Run:

```powershell
npm ci
npm run build
npm run dev
```

Connect a Testnet wallet, open a known circle ID or create a test circle, and compare displayed circle/member/round/accounting values against contract views and the Testnet explorer. Use native Testnet NEAR only. Do not use real funds.

## References

- [NEAR CLI-RS user guide](https://github.com/near/near-cli-rs/blob/main/docs/GUIDE.en.md)
- [NEAR Testnet RPC](https://rpc.testnet.near.org)
- [NEAR Testnet explorer](https://testnet.nearblocks.io)
