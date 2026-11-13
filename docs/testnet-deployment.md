# NEAR Testnet deployment guide

This guide describes a future manual Testnet deployment. **No deployment has been performed. Do not use Mainnet.** Deployment requires the account owner's wallet authorization and an explicit review of the account before sending the deploy transaction.

## Deployment status established from this repository

The repository does not establish a deployed Testnet contract account ID. The README and this guide explicitly state that no Testnet deployment occurred; the frontend and Edge Function examples use placeholders, and repository history contains only the initial project commit. No deployment transaction or deployed-account metadata is recorded. A local NEAR Sandbox account or test is not evidence of a Testnet deployment. Consequently, the expected account ID is **not established**, and no current deployed code can be compared with this source. Do not set `VITE_CONTRACT_ID` or `NEAR_CONTRACT_ID` to a guessed value.

The current source does support chat membership checks: `get_members(circle_id)` is a public view that returns member records including `account_id`; `authorize-circle-chat` compares that field with the server-managed wallet identity. It also returns a member `status`, but the current authorization checks membership by presence and does not filter by status. The contract's `new()` initializer takes no arguments. A successful deployment must therefore include initialization, as the deploy command below does. For an account with an existing contract or state, stop; the current contract has no migration/upgrade path suitable for reuse.

## Contract behavior relevant to the first deployment

`MemberView` from `get_members(circle_id)` returns:

- `account_id` (NEAR account ID), `payout_position` (u32).
- `collateral_remaining_yocto`, `total_contributions_yocto`, `total_payouts_yocto` (decimal strings).
- `on_time_payments`, `late_payments` (u32).
- `status` (`Joined`, `RefundPending`, or `Refunded`), `cancellation_approved` (boolean), `cancellation_refund_yocto` (decimal string).
- `deposit_refund`, `cancellation_refund`, `storage_allowance_refund` transfer views. Each contains `id` (u64), `kind`, optional `recipient_id`, decimal-string `amount_yocto`, `state`, and `attempts` (u32).

There are no `Inactive`, `Pending`, `Suspended`, or revoked member states. `RefundPending` and `Refunded` describe a refund transfer: members remain in the roster, including after completion or cancellation. There is no member-removal operation. Chat authorization therefore checks exact account presence in the returned roster; filtering refund statuses would incorrectly turn financial transfer state into a chat revocation rule. The Edge Function also does not call `get_circle`, so it does not filter by `Open`, `Active`, `Paused`, `Completed`, `Cancelled`, or `ClockExhausted`; historical members of any existing circle remain eligible. This is the current policy, not a revocation feature. If chat access should end when a circle is cancelled/completed or a refund is paid, define that rule separately before changing authorization.

Initialization is `new()` with **no arguments**. It is guarded by NEAR SDK's one-time `#[init]` check, but there is no owner restriction: the first successful call can initialize the contract. It initializes schema version 2, next circle and transfer IDs to 1, pending outgoing and funded storage counters to zero, and an empty circle map. It creates no circle and no member. Deploy and initialize atomically with `with-init-call new` to avoid leaving an uninitialized contract that another account could initialize first.

## Safety requirements

- Use a dedicated account ending in `.testnet` that you control and can authorize from a NEAR Testnet wallet.
- Do not share, paste, or commit a seed phrase, private key, key file, or wallet credentials. `near-cli-rs` can use its interactive Testnet keychain flow.
- Before deploying, inspect the target account on Testnet and verify its owner/access keys and contract state. The account must be intended for this project and have no existing contract code or state. If it has ever been used for another contract or prototype, stop and choose a fresh account; this contract has no migration method.
- The deploy command replaces contract code. Never deploy over an account whose state or ownership you have not verified.
- This project has no Mainnet deployment path or authorization.

## 1. Create or select and verify a Testnet account

Choose a dedicated, fresh account. If you create one with the CLI faucet helper, this command creates a Testnet account and saves its generated key in the OS keychain; it submits an account-creation transaction:

```powershell
near account create-account sponsor-by-faucet-service <your-account>.testnet `
  autogenerate-new-keypair `
  save-to-keychain `
  network-config testnet `
  create
```

Alternatively, create/select the account in your Testnet wallet, then import it into the CLI using the wallet authorization flow (do not enter a seed phrase):

```powershell
near account import-account using-web-wallet <your-account>.testnet `
  network-config testnet `
  save-to-keychain
```

Approve the CLI account/key authorization in the Testnet wallet. For an already imported account, skip account creation/import and continue. Verify that the account and its full-access key belong to you, and check its balance:

```powershell
near account view-account-summary <your-account>.testnet network-config testnet now
near account list-keys <your-account>.testnet network-config testnet now
near tokens view-near-balance <your-account>.testnet network-config testnet now
```

Check the public keys against the account in your wallet; stop if the owner/key relationship is unclear. These view commands do not submit a transaction.

## 2. Build and record the optimized Wasm

From the repository root in PowerShell:

```powershell
cargo fmt --manifest-path contract/Cargo.toml -- --check
cargo test --manifest-path contract/Cargo.toml --locked
cargo near build non-reproducible-wasm --manifest-path contract/Cargo.toml --locked --no-abi
Get-FileHash contract/target/near/kutu_dao_contract.wasm -Algorithm SHA256
```

The target is `wasm32-unknown-unknown` (pinned in `contract/rust-toolchain.toml`). `cargo-near` uses Cargo's optimized `release` profile by default and runs `wasm-opt -O` on the output. `--no-abi` skips ABI generation/embedding only; it does not disable Wasm optimization. The output is `contract/target/near/kutu_dao_contract.wasm`.

Build verified for this handover: 228,215 bytes, SHA-256 `c05d249dd7dd6e6a2587ad2d0f420424479b1df78ef3cdbb8d5f75630871735b`. The artifact was freshly rebuilt and the log confirmed `wasm-opt -O` ran. Rebuild and recalculate the hash for any source or toolchain change; deploy only the artifact whose hash you reviewed.

Record the output hash alongside the release notes and verify it again before deployment. This command builds optimized Wasm but is labeled non-reproducible by `cargo-near`; matching hashes should be confirmed by rebuilding in the intended release environment. The Wasm is a build artifact and is ignored by Git; rebuild it from the recorded source revision. The current Windows build skips ABI generation with `--no-abi` because the ABI generator cannot link NEAR host functions in the Windows environment.

The artifact is a `non-reproducible-wasm` build from this working tree. Sandbox is a local test network, not Testnet. The most recent recorded Sandbox run in [development.md](development.md) passed against a separately built 228,159-byte artifact; the current 228,215-byte artifact was not Sandbox-tested in this run because WSL returns `Wsl/E_ACCESSDENIED`. Do not claim the current artifact has passed Sandbox until the WSL2 test is rerun against its hash.

## 3. Inspect the target account and estimate funding

Inspect the selected account using Testnet-only CLI commands:

```powershell
near contract inspect <your-account>.testnet network-config testnet now
near contract view-storage <your-account>.testnet all as-json network-config testnet now
```

Confirm the account ID and access keys are yours, the account has no deployed contract code, and it has no contract state. If inspection is unclear, the account has any existing contract/state, or you cannot verify ownership, stop and use a freshly created account instead.

The 228,215-byte code requires account storage stake, in addition to account/access-key/state storage and transaction gas. The exact Testnet storage rate, total state usage, and gas fee were not queried in this preparation, so a precise required balance cannot be stated. Check the account balance immediately before deployment and proceed only if the CLI accepts the transaction with sufficient remaining balance. NEAR transactions fail if the account cannot cover its state storage requirement; gas prices are dynamic. [NEAR account storage overview](https://github.com/near/nearcore/blob/master/docs/DataStructures/Account.md), [NEAR fees and state stake](https://github.com/near/nearcore/blob/master/docs/Economics/Economics.md).

## 4. Deploy and initialize in one Testnet transaction

Only after confirming ownership, fresh contract state, and adequate balance, replace `<your-account>.testnet` with that verified ID. From the repository root:

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

`sign-with-keychain` signs using the account key in the operating-system keychain; the CLI does not necessarily open a wallet-extension approval for this transaction. If you imported through the web-wallet flow, that wallet authorization was used to authorize/import the account key. Review the CLI's selected account, Testnet network, artifact path, initializer, and transaction summary before sending. If you use a Ledger-backed key, select the CLI's `sign-with-ledger` flow and approve the details on the device. Never use plaintext-key or seed-phrase command options. This deploy command and the `new()` initializer are state-changing; do not proceed if the target details differ.

Keep the successful transaction ID, deployed account ID, reviewed Wasm SHA-256, date, and account summary together in the release record. A successful local build is not evidence of a successful Testnet deployment.

Verify and preserve the finalized transaction result:

```powershell
near transaction view-status <deployment-transaction-hash> network-config testnet now
```

Record these fields in the release record (do not include any key material): contract account ID, transaction hash, finalized status, deployment date, local Wasm SHA-256, artifact byte size, and the account's post-deployment balance/storage summary. The transaction can also be opened at `https://explorer.testnet.near.org/transactions/<deployment-transaction-hash>`.

## 5. Verify the deployed artifact and contract views

After deployment, use the same explicit Testnet CLI network selection. First confirm that the contract is initialized, inspect its code/state, and call `get_members` for a real circle ID that exists on that contract:

```powershell
near contract inspect <your-account>.testnet network-config testnet now
near contract view-storage <your-account>.testnet all as-json network-config testnet now
near contract call-function as-read-only <your-account>.testnet get_members json-args '{"circle_id":1}' network-config testnet now
```

The first `new()` initialization creates no circles. Create/join a disposable test circle on this contract before querying `get_members`; replace `1` with that existing circle ID. A nonexistent circle causes a contract panic and does not prove the view method is missing. The call must return the expected member records, including `account_id` and the documented status.

To compare the deployed Wasm bytes with the reviewed local build without submitting a transaction, query Testnet `view_code` and compare its base64 code to the local artifact:

```powershell
$contractId = '<your-account>.testnet'
$requestBody = @{
  jsonrpc = '2.0'
  id = 'verify-contract-code'
  method = 'query'
  params = @{ request_type = 'view_code'; finality = 'final'; account_id = $contractId }
} | ConvertTo-Json -Depth 5
$deployed = Invoke-RestMethod -Method Post -Uri 'https://rpc.testnet.near.org' -ContentType 'application/json' -Body $requestBody
$localCode = [Convert]::ToBase64String([IO.File]::ReadAllBytes('contract/target/near/kutu_dao_contract.wasm'))
if ($localCode -cne $deployed.result.code_base64) { throw 'Deployed code differs from the local artifact.' }
'Deployed code exactly matches the local artifact.'
Get-FileHash contract/target/near/kutu_dao_contract.wasm -Algorithm SHA256
```

This comparison requires a successful RPC response with `result.code_base64`; stop and investigate any RPC error. Keep the account ID, local SHA-256, CLI transaction result, and verification output together as the deployment record.

## 6. Configure the frontend and chat Edge Functions

Copy `frontend/.env.example` to `frontend/.env.local`, then set:

- `VITE_CONTRACT_ID` to the verified deployed `.testnet` account ID.
- `VITE_SUPABASE_URL` to the test project's Supabase project URL.
- `VITE_SUPABASE_ANON_KEY` to its public anon/publishable key only.

In the test project's Edge Function Secrets, set:

- `NEAR_CONTRACT_ID` to the **same** verified `.testnet` contract account ID as `VITE_CONTRACT_ID`.
- `NEAR_CHAT_AUTH_RECIPIENT` to `kutu-dao-chat`. This is a stable application identifier embedded in the NEP-413 challenge; it is **not** the contract account ID. The challenge and verifier require the same value.
- `CHAT_ALLOWED_ORIGINS` to the exact deployed frontend origin (scheme and host, no path or trailing slash).

Do not put Supabase secret/service-role keys in the frontend. More setup and test-project migration steps are in [supabase-chat.md](supabase-chat.md). The RPC URL is fixed to `https://rpc.testnet.near.org`; wallet connection, account retrieval, signing, signing out, and transaction submission explicitly use Testnet.

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
