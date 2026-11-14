# Development environment setup (Windows)

Checked against official instructions on 2026-11-13. Open a new PowerShell window after installing tools so the updated PATH is loaded.

## Current verification status — 2026-11-14

These checks are distinct from the hosted Testnet deployment reported by the owner:

- **Native Rust tests:** `cargo test --manifest-path contract/Cargo.toml --locked` passed 29 tests.
- **Optimized WASM build (2026-11-13):** `cargo near build non-reproducible-wasm --manifest-path contract/Cargo.toml --locked --no-abi` produced the 228,215-byte optimized artifact at `contract/target/near/kutu_dao_contract.wasm`; SHA-256 `c05d249dd7dd6e6a2587ad2d0f420424479b1df78ef3cdbb8d5f75630871735b`. The recorded build ran `wasm-opt -O`.
- **NEAR Sandbox integration:** `AWS_LC_SYS_PREBUILT_NASM=0 cargo test --locked --features sandbox-tests --test sandbox -- --nocapture` passed 1 test on 2026-11-14 in Ubuntu under WSL2. The test loads the project artifact via `include_bytes!`; this is local Sandbox execution, not Testnet execution.
- **Frontend:** `npm test` passed 11 tests (2 frontend and 9 shared/auth/membership); `npm run build` passed.
- **Local Supabase database:** the empty-database migration and pgTAP suite passed 34 tests, and the anonymous REST security smoke test passed. These are local database checks, not hosted Supabase integration.
- **Testnet:** the owner reports the contract was deployed and initialized at `kutunear.testnet` (version 0.1.0; deployment transaction `HDgoZ6pabkwVPSDHXCvyj9PzLTyUff78P68CdXrQMrKA`). The owner also reports Circle #1 was joined by two members, completed two contribution rounds, and reached `Completed`, which the frontend displayed. This is an owner-reported live demonstration and was not independently re-queried during this documentation update. Hosted Supabase chat remains unverified. See [testnet-deployment.md](testnet-deployment.md).

The dated verification notes later in this file are retained as historical records. Where they say Sandbox, Supabase, or WSL checks were unavailable, those statements describe that earlier run and are superseded by this current status.

## Already available

- Node.js 24.21.0
- npm 11.19.0

The frontend requires Node.js 22.12 or newer.

## Install the Rust/NEAR toolchain

1. Install Rust through the official [Rust Windows installer](https://www.rust-lang.org/tools/install). Use the default MSVC toolchain. If prompted, install the Microsoft C++ Build Tools required by Rust on Windows.
2. In a new PowerShell window, add the WebAssembly compilation target:

   ```powershell
   rustup target add wasm32-unknown-unknown
   ```

3. Install the official NEAR Rust contract build helper (PowerShell installer):

   ```powershell
   irm https://github.com/near/cargo-near/releases/latest/download/cargo-near-installer.ps1 | iex
   ```

4. Install the current NEAR CLI-RS (the official CLI guide lists the npm distribution). npm's install-script policy must allow this package's binary installer:

   ```powershell
   npm install --global --allow-scripts=near-cli-rs near-cli-rs@latest
   ```

5. Confirm the commands are available:

   ```powershell
   rustc --version
   cargo --version
   rustup target list --installed
   cargo near --help
   near --version
   ```

The contract currently pins `near-sdk` 5.30.1. Its upstream release metadata specifies Rust 1.95.0 as the minimum supported compiler, so the stable toolchain installed from rustup must be at least that new. If it is older, update it with `rustup update stable` before building.

## Verified local versions

Installed and confirmed for this workspace on 2026-11-13:

- Node.js 24.21.0 and npm 11.19.0
- Rust/Cargo 1.100.0 and `wasm32-unknown-unknown`
- `cargo-near` 0.23.0
- `near-cli-rs` 0.31.1

Rustup added its binaries to `%USERPROFILE%\.cargo\bin`. If an existing terminal cannot find `rustup`, `rustc`, `cargo`, or `cargo near`, open a new PowerShell window (or temporarily prepend that directory to `$env:PATH`).

## Install frontend dependencies

From the project root:

```powershell
npm install
npm run dev
```

The frontend uses `@hot-labs/near-connect` with `network: 'testnet'` and a Testnet-only wallet feature filter. Wallet account lookup, signing, and sign-out explicitly pass `network: 'testnet'`. During local development, reads go through `/api/near-rpc`; Vite forwards them to the optional `NEAR_RPC_URL` from `frontend/.env.local`, or to public Testnet RPC if it is unset. The URL is server-side development configuration and must not use the `VITE_` prefix. Production builds do not require it and currently read from the public Testnet RPC directly; no production RPC proxy is configured. `VITE_CONTRACT_ID` is set to `kutunear.testnet` in the checked-in example. Do not place keys or credentials in frontend variables. See [testnet-deployment.md](testnet-deployment.md).

## Contract development checks

Run the native Rust unit tests:

```powershell
cargo test --manifest-path contract/Cargo.toml --locked
```

The contract's `.cargo/config.toml` enables AWS-LC's upstream prebuilt Windows x86-64 assembly objects for the native unit-test dependency tree. This avoids installing NASM just to run the tests. If you prefer to compile those objects from source, install NASM from the [official NASM site](https://www.nasm.us/) and set `AWS_LC_SYS_PREBUILT_NASM=0` for that shell.

Build the current create/join contract to Wasm:

```powershell
cargo near build non-reproducible-wasm --locked --no-abi
```

`--no-abi` is currently needed on this Windows setup because the native ABI-generation step tries to link NEAR host functions into a Windows executable. The Wasm build itself succeeds. The frontend uses the known JSON view interface directly and does not consume generated ABI data.

### Current create/join test results

On 2026-11-13, `cargo test --locked -- --nocapture` passed 17 tests, including transfer success, failure, permissionless retry, duplicate callback, stale-attempt callback, duplicate retry, and external callback rejection coverage. The maximum-capacity case creates a 10-member circle and successfully joins 10 accounts, checking all permanent positions and each round's preallocated 10 payment and collateral slots. Separate tests reject inadequate storage payments at both entrypoints. Native test VM storage measurements were 4,298 bytes for creation and 1,740 bytes total for ten joins. At that test VM's storage price of 10^19 yoctoNEAR per byte, those charges are 0.04298 NEAR and 0.01740 NEAR respectively.

The native unit-test VM reported 25,502,516,980,607 gas (25.50 Tgas) for creation, 255,722,142,392,553 gas (255.72 Tgas) across ten join transactions, and 25,662,950,056,861 gas (25.66 Tgas) for the highest individual join. Treat these as native host-meter figures: they are not actual optimized-Wasm runtime measurements.

## Testnet credentials

Do not put private keys, seed phrases, or wallet credentials in this repository or in frontend environment variables. Use the wallet's interactive authorization flow. Deployment instructions and explicit fresh-account safeguards are in [testnet-deployment.md](testnet-deployment.md). Never deploy this MVP to Mainnet.

## Optimized-Wasm Sandbox verification (WSL2)

The native Windows unit tests cannot measure NEAR runtime gas for the optimized Wasm. The opt-in integration test at `contract/tests/sandbox.rs` uses the lockfile's `near-workspaces` 0.24.0 to run the compiled Wasm in NEAR Sandbox. That release documents sandbox support on Linux x86-64/aarch64 and macOS ARM64, so run it inside Ubuntu on WSL2, not in Windows PowerShell. The test covers circle creation, an explicit creator join, multiple joins, duplicate rejection, a 10-member circle, and full-circle rejection. It prints actual sandbox runtime gas and contract account storage deltas. [near-workspaces requirements](https://github.com/near/near-workspaces-rs#requirements)

**Runner maintenance note:** These steps use the runner already pinned in `contract/Cargo.lock` so the test can be run without changing dependencies. NEAR's developer-experience tracker currently marks `near-workspaces-rs` as deprecating and lists `near-sandbox-rs` as stable. Reassess the test runner before expanding the suite; this WSL setup does not migrate it. [NEAR DevEx status](https://github.com/near/devex)

### Step 1: Open the existing project from Ubuntu

No project copy or clone is needed. WSL mounts the existing Windows `C:` drive at `/mnt/c`. Open Ubuntu and run:

```bash
cd "/mnt/c/Users/ngwanyang/Desktop/CODEX-TEST/Near Hackathon/contract"
pwd
```

The printed directory should end in `/Near Hackathon/contract`. Run the build and test commands from this directory. Keep Windows and WSL builds sequential: both use the existing `contract/target/` folder.

### Step 2: Install only the Linux build prerequisites

In Ubuntu, install the native compiler/build tools, OpenSSL development files, and NASM:

```bash
sudo apt update
sudo apt install --no-install-recommends build-essential cmake curl libssl-dev nasm pkg-config
```

NASM is needed for native Rust test dependencies in Linux. The contract's `contract/.cargo/config.toml` selects prebuilt NASM objects for the existing Windows setup; the test command below overrides that setting for this Linux process so Linux builds its own assembly. This does not edit the Windows configuration.

No Node.js, NEAR CLI, or manually installed Sandbox executable is needed for these contract tests. `near-workspaces` downloads and launches the compatible Sandbox binary when the test runs; that first run needs network access.

### Step 3: Check or install the Linux Rust toolchain

The Rust installation in Windows is separate and cannot be used as Ubuntu's Linux toolchain. First check whether Rust is already available in Ubuntu:

```bash
source "$HOME/.cargo/env" 2>/dev/null || true
rustc --version
cargo --version
rustup target list --installed
```

The pinned `near-sdk` 5.30.1 requires Rust 1.95.0 or newer. If `rustup` is missing, install the minimal stable Rust toolchain in the Ubuntu user account:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
source "$HOME/.cargo/env"
```

If Rust is present but older than 1.95.0, update only the Ubuntu stable toolchain:

```bash
rustup update stable
rustup default stable
```

Add the contract's Wasm target if it is not already listed:

```bash
rustup target add wasm32-unknown-unknown
```

Confirm the compiler meets the minimum:

```bash
rustc --version
```

### Step 4: Install `cargo-near` in Ubuntu

Check first:

```bash
cargo near --version
```

If unavailable, use the official Linux installer, which installs into the Ubuntu user's Cargo bin directory:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/near/cargo-near/releases/latest/download/cargo-near-installer.sh | sh
source "$HOME/.cargo/env"
cargo near --version
```

This does not replace or modify the Windows `cargo-near` installation. The install method is from the [official cargo-near project](https://github.com/near/cargo-near#installation).

### Step 5: Build and run the optimized-Wasm Sandbox test

From `contract/`, build the optimized Wasm and then run the single integration test:

```bash
cargo near build non-reproducible-wasm --locked --no-abi
AWS_LC_SYS_PREBUILT_NASM=0 cargo test --locked --features sandbox-tests --test sandbox -- --nocapture
```

The first test run compiles the optional `near-workspaces` test dependencies and downloads/starts Sandbox. Both can take several minutes. `--locked` uses the checked-in `Cargo.lock`. `--no-abi` skips the Windows-specific native ABI linking problem; it still builds the optimized contract Wasm consumed by the Sandbox test. Do not pass `--all-features` or run this opt-in test from Windows PowerShell.

### Step 6: Read the measurements

On success, the test prints a line like:

```text
SANDBOX_WASM_METRICS create_storage_bytes=... create_gas=... ten_join_storage_bytes=... ten_join_gas=... max_individual_join_gas=... duplicate_rejected=true full_rejected=true
```

- `create_gas` is gas burnt for the 10-member circle creation transaction and its receipts in Sandbox.
- `ten_join_gas` is the sum across ten separate successful join transactions; `max_individual_join_gas` is the largest single join result.
- `create_storage_bytes` is the contract account's storage usage increase when that circle is created. `ten_join_storage_bytes` sums the account storage increases across its ten joins.
- The storage fees the contract charged are the runtime's measured byte deltas multiplied by its storage byte price. The test output reports bytes; the prior native test figures are not substitutes for these values.
- `duplicate_rejected=true` and `full_rejected=true` confirm those calls failed as required. The test also checks external invocation of the private callback is rejected.

These are optimized-Wasm executions under the local Sandbox runtime and are the right measurements for reviewing the 10-member design. They are not Testnet transaction records, and Sandbox's protocol/runtime configuration can differ from Testnet. Keep the approved limit at 10 unless the measurements are reviewed and a change is explicitly agreed. The earlier **native** host-meter gas and mock storage results are listed separately above and must not be reported as Sandbox results.

### Verified WSL2 Sandbox results

Executed on 2026-11-13 in Ubuntu 26.04.1 LTS under WSL2, with the optimized Wasm built by `cargo near build non-reproducible-wasm --locked --no-abi`. The single `optimized_wasm_create_join_rejection_and_max_capacity_measurements` integration test passed (1 passed, 0 failed). It deployed and executed the compiled Wasm in Sandbox and confirmed explicit creator joining, duplicate rejection, ten successful joins, and eleventh-member rejection. The test also confirmed an external account cannot invoke the private transfer callback.

```text
SANDBOX_WASM_METRICS create_storage_bytes=4313 create_gas=5872170559896 ten_join_storage_bytes=2020 ten_join_gas=64661087440370 max_individual_join_gas=6786018281480 duplicate_rejected=true full_rejected=true
```

That is 5.87 Tgas for creation, 64.66 Tgas total across ten joins, and 6.79 Tgas for the highest individual join. Sandbox reported a 4,313-byte storage increase for creation and 2,020 bytes summed across ten successful joins. At the Sandbox storage price of 10^19 yoctoNEAR per byte, the corresponding storage charges are 0.04313 NEAR and 0.02020 NEAR. The 10-member limit was retained: these measurements do not show a reason to reduce it. They are local Sandbox results, not Testnet measurements. Native unit-test figures above remain a distinct, non-runtime measurement.

## Historical smart-contract audit before lifecycle implementation

> This section records the earlier create/join-only audit and is superseded by the lifecycle implementation update below. Statements about financial entrypoints being absent describe the code at that earlier audit point, not the current contract.

## Historical bounded code review (not a security audit)

This is a historical, bounded internal code review performed on 2026-11-13, not a security audit or third-party assessment. Its description is a snapshot of the implementation at that time and is superseded by the lifecycle implementation and verification sections later in this document. The contract now implements the contribution, default, cure, payout, refund, retry, and cancellation methods documented in the current architecture and source.

### Findings by severity

- This historical review did not establish security assurance. Its scoped observations applied to create/join/storage-refund paths at that snapshot only; they are not conclusions about the current implementation.
- **Medium — first deadline can be zero or in the past.** `Circle::open` rejects a zero interval and checks arithmetic overflow, but does not validate `first_deadline_ns` against the current block timestamp. If default processing is later implemented using that deadline, a circle can become Active with its first deadline already elapsed. Proposed remediation before implementing defaults: agree and enforce whether creation must specify a future deadline and whether the final join must occur before it, or whether deadlines instead begin on activation. No rule was silently selected here.
- **Medium — optimized-WASM failure path is not exercised end to end.** Native tests simulate a failed promise result and verify Failed → retry Pending → Succeeded, same ID/recipient/amount, duplicate retry rejection, stale-attempt rejection, and pending-total restoration. The Sandbox test runs optimized Wasm for create/join and verifies rejection cases and callback privacy, but does not force an actual transfer receipt failure or inspect transfer state after callback. Add a Sandbox failure/retry scenario with a contract-level inspection method or equivalent test-only observation before relying on receipt-level behavior for financial transfers.
- **Low — query surface is incomplete.** The only read method is `get_storage_price_yocto`; there are no circle/member/round views. The future frontend cannot yet verify on-chain membership positions or financial state through a supported view API. Add bounded, read-only views before frontend integration.
- **Low — approved target model and current Borsh model differ.** Current stored structs do not yet include all documented pause/default/cancellation fields, and current enum variants do not yet include `Cancelled` or `CancellationRefund`. This is expected for incremental development, but the architecture document must be read as the target design, not a description of deployed/current serialized state. Resolve this explicitly when those phases are implemented; avoid migrating or changing serialized enums without a versioning plan.

### Review of implemented paths

- Circle creation validates the bounded configuration in `Circle::open`, parses amounts as `u128`, preallocates no more than ten rounds, measures flushed storage growth, requires separately attached storage funds, and assigns an increasing ID only on success. NEAR transaction rollback means an underfunded panic does not persist the provisional write or consume the ID.
- Joining checks the circle exists, requires at least the configured collateral, and delegates Open/full/duplicate/exact-collateral/position checks to `append_member`. The final successful join activates the circle. There is no leave or rule-edit method, so the current membership order cannot be changed through the exposed API.
- Join storage is measured independently from collateral. Excess is saved to a fixed creator/member slot and returned to the creator or joining account; the transfer amount is fixed before the promise. The fee for occupied storage is not refunded.
- Transfer operation IDs are allocated from a checked global counter. A slot enters Pending before scheduling. Retry is permissionless but requires Failed and reuses the stored ID, amount, and recipient slot while incrementing the attempt. The callback is private, checks the predecessor, requires exactly one result, verifies kind/ID/attempt/Pending state through `settle`, and removes exactly the saved amount from the pending total. Failure leaves the transfer Failed and its claim amount intact. This code-level reasoning is covered by native state-machine tests; receipt-level failure behavior is the Medium finding above.
- There are no end-user withdrawal, contribution, or payout entrypoints at present, so unauthorized use, over-withdrawal, duplicate contribution/payout, and contribution/payout accounting questions cannot be meaningfully tested yet. The only arbitrary-user methods are intentionally permissionless create/join and failed-storage-refund retry; the private callback is not permissionless.
- Account registration is not used for NEAR native transfers. The storage recipient must be an existing account because only an account that can sign a join/create transaction is saved as creator/member. A failed transfer remains retryable if the destination later becomes receivable.

### Coverage and test results

Added native test `create_and_join_keep_collateral_storage_cost_and_excess_refund_separate`. It checks that collateral remains exactly the configured amount, measured storage cost is recorded separately, creation and join overpayments have fixed recipients and exact refund amounts, and the pending/storage totals reconcile across both operations. No production contract behavior or approved rule was changed.

On 2026-11-13, native `cargo test --manifest-path contract/Cargo.toml --locked` passed **18 tests, 0 failed** (plus 0 doc tests). It covers create/join limits and ordering, insufficient storage, storage accounting, transfer success/failure/retry, duplicate and stale callbacks, external callback rejection, retry immutability, and maximum-capacity native model measurements. Native promise-result tests simulate callbacks; they are not NEAR receipt execution.

On 2026-11-13, `cargo near build non-reproducible-wasm --locked --no-abi` succeeded and the optimized-WASM Sandbox integration passed **1 test, 0 failed**. Measurements remained `create_storage_bytes=4313`, `create_gas=5872170559896`, `ten_join_storage_bytes=2020`, `ten_join_gas=64660870232786`, and `max_individual_join_gas=6785909677688`. Duplicate and full joins were rejected. This is an actual optimized-Wasm run in local Sandbox, not Testnet. The new test changed only native test code, so production Wasm and these runtime measurements are unchanged.

### Readiness decision

**Not ready for NEAR Testnet financial integration or deployment.** The implemented create/join/storage-refund slice passed its current native and optimized-WASM checks, but essential financial entrypoints are absent, the first-deadline rule needs a decision before default handling, and the asynchronous failure path still needs an actual Sandbox receipt-level test. Do not deploy or accept financial commitments until these blockers are resolved and the complete financial lifecycle receives its own review.

## Official references

- [Rust installation](https://www.rust-lang.org/tools/install)
- [NEAR contract quickstart and tool prerequisites](https://docs.near.org/smart-contracts/quickstart)
- [NEAR CLI installation](https://docs.near.org/tools/cli)
- [near-sdk-rs repository and minimum Rust version](https://github.com/near/near-sdk-rs)

## Financial lifecycle implementation update — 2026-11-13

The previously approved financial lifecycle design has now been implemented in `contract/src/model.rs` and `contract/src/lib.rs`. This update supersedes the earlier create/join-only readiness finding above. No frontend changes, Testnet deployment, or real financial transactions were made.

### Implemented behavior

- Activation-based first deadline, with `payment_interval_ns` as the initial window. Each later round starts its interval only after the prior payout succeeds.
- Exact, member-derived, once-per-round contributions; permissionless overdue default processing in immutable join order; exact collateral substitution or a no-deduction pause; exact cure from only the designated member.
- Payout recipient/amount derived from round order and target size; asynchronous saved transfer state; completion and collateral refunds.
- Unanimous pre-payout cancellation with revocable consent, frozen per-member refund entitlement, and fixed-recipient claims. Creator can close an empty Open circle.
- Separate measured storage charges and member collateral/contribution accounting; private callback and permissionless fixed-slot retry rules retained.
- Read-only views for circle, members, rounds, member accounting, transfer state, and storage price; structured events.

### Changes and security notes

- Contract `schema_version` is now 2. Borsh state layouts/enums changed as part of this pre-deployment implementation. There is no migration method. Use a fresh contract account for a first Testnet deployment; do not update an older prototype's code while retaining its state.
- Paused default cure is a new direct contribution, not a collateral deduction. Full collateral substitution reduces collateral and adds the same amount to that member's contribution slot. Cancellation computes `remaining collateral + direct contributions held + collateral substitutions held`, preventing double counting.
- Outgoing transfer calls reserve their saved amount and recipient before a promise. Callback validates private predecessor, exactly one promise result, ID, attempt, and Pending state. Retry only starts from Failed and reuses ID, recipient, and amount with incremented attempt. Native tests cover failed transfers, duplicate/stale callback attempts, and preservation of original entitlement.
- A destination may remain unable to receive funds. In that case the amount remains an on-chain Failed claim; retries cannot redirect it. If a member never consents, cancellation remains unavailable under the approved unanimity rule. There is no administrator recovery.
- Sandbox verified actual optimized-Wasm success callbacks, including a payout at 10-member capacity and cancellation refund callbacks. It did not yet force an actual receipt-level transfer failure; failure/retry mechanics are verified by native promise-result tests. That is an explicit remaining Sandbox coverage gap, not a verified runtime claim.

### Tests and measurements

Windows native checks on 2026-11-13:

```text
cargo fmt --manifest-path contract/Cargo.toml -- --check  PASS
cargo test --manifest-path contract/Cargo.toml --locked    23 passed, 0 failed
```

Ubuntu WSL2 optimized-Wasm Sandbox check on 2026-11-13:

```text
cargo test --locked --features sandbox-tests --test sandbox -- --nocapture
1 passed, 0 failed
SANDBOX_WASM_METRICS create_storage_bytes=4545 create_gas=6316172809500 ten_join_storage_bytes=2540 ten_join_gas=70591798092487 max_individual_join_gas=7522089102154 duplicate_rejected=true full_rejected=true max_capacity_payout_callback=true
SANDBOX_FINANCIAL_LIFECYCLE round0_payout=2000 recipient_position=1 callback_finalized=true
```

The maximum-capacity run executed optimized contract Wasm in NEAR Sandbox. It exercised 10 successful joins, rejected an eleventh member and duplicate membership, processed 10 exact contributions, and observed the successful payout callback to position 1. The separate financial scenarios exercised nonmember contribution rejection, a short-collateral pause, wrong-account/short/exact cures, ordered resume to a ready round, full collateral coverage, unanimous cancellation, and successful fixed-recipient refund callbacks.

The Sandbox block timestamp initially advanced less than the test's one-hour fixture deadline. The test fixture now uses a 10-second payment interval and 100-block fast-forward; the test asserts the observed block timestamp is past the exposed deadline before processing defaults. This changes test data only; contract deadlines and business rules are unchanged.

At the Sandbox price of 10^19 yoctoNEAR per byte, measured storage is 0.04545 NEAR for 10-member circle creation and 0.02540 NEAR summed across ten successful joins. Gas is reported in raw gas units: 6.316 Tgas for creation, 70.592 Tgas summed across joins, and 7.522 Tgas for the largest individual join. The configured maximum remains 10; these results do not justify changing it. Local Sandbox figures are not Testnet transaction measurements.

### Remaining review and release status

- The actual failed-transfer and retry gap is now closed by the Sandbox scenario below. Native tests remain useful for deliberately injected stale/duplicate callback results, which are not naturally emitted by Sandbox after a single promise chain.
- Recheck worst-case callback gas and storage neutrality after any schema or callback change. The current maximum-capacity payout callback succeeds in Sandbox, but it does not cover every worst-case refund/failure combination.
- Historical release-status note (2026-11-13): at that point, Testnet integration/deployment remained outstanding. The owner subsequently reported deployment and a completed Circle #1 demonstration; see the current status at the start of this document.

## Actual Sandbox failed-transfer and retry verification — 2026-11-13

The transfer implementation was inspected before testing. `finalize_payout` fixes the operation ID, immutable member-position recipient, amount, and Pending state before scheduling `Promise::transfer`. The private callback accepts only the contract as predecessor and one promise result; `PendingTransfer::settle` requires the same ID, current attempt, and Pending state. On failure, callback marks the saved transfer Failed and the round `TransferFailed`, subtracts only the in-flight reservation, and leaves the round's contribution records unchanged. `retry_transfer` is permissionless by design but requires the exact failed operation ID and slot; caller cannot set recipient or amount. Retry increments the attempt and reuses the saved ID/recipient/amount. Success finalizes once and advances the circle.

### Real failure method and observations

The optimized-Wasm Sandbox integration creates a payout recipient as a named child account, joins it at position 1, settles its storage-allowance refund, then deletes the account with a successful Sandbox delete-account transaction. The test confirms the account no longer exists before calling `finalize_payout`. The payout targets that fixed, now-nonexistent account. `ExecutionFinalResult::receipt_failures()` contains a failed receipt whose executor is the deleted payout recipient. Meanwhile, the payout transaction's callback completes successfully and the contract view reports the saved payout as `Failed`.

After the failed receipt and callback, the test verifies:

- Transfer ID is unchanged, amount is `2,000` yoctoNEAR, saved recipient is the deleted member account, attempt is 1, and transfer state is Failed.
- Round is `TransferFailed`; collected amount remains 2,000 yoctoNEAR; circle `current_round` remains 0 and successful payout count remains 0.
- Member collateral remains the configured 3,000 yoctoNEAR.
- Contract storage usage does not increase during the failed payout/callback.
- A permissionless retry using an incorrect operation ID fails without changing the Failed claim or attempt.

The root Sandbox account then recreates the exact same named account ID. A permissionless retry with the saved operation ID succeeds. The recreated recipient's balance increases by exactly 2,000 yoctoNEAR from its pre-retry balance. The transfer has the same ID, attempt 2, and Succeeded state; round 0 is Finalized; the circle advances to round 1 with successful payout count 1; member collateral remains 3,000 yoctoNEAR and the member payout total is 2,000 yoctoNEAR. A retry after success fails and a second balance check confirms no extra transfer. Contract storage usage remains unchanged by both failure handling and retry.

Sandbox cannot be asked to emit a second genuine callback for the same promise receipt. Duplicate and stale callback replay protections are therefore covered by existing native state-machine tests; Sandbox verifies a single genuine callback per real failed/successful transfer and rejects duplicate retry attempts at the public entrypoint.

### Results

```text
AWS_LC_SYS_PREBUILT_NASM=0 cargo test --locked --features sandbox-tests --test sandbox -- --nocapture
1 passed, 0 failed; finished in 56.32s
SANDBOX_FAILED_TRANSFER_RETRY transfer_id=41 failed_attempt=1 retry_attempt=2 amount_yocto=2000 recipient_balance_delta=2000 failed_attempt_gas=5208730356395 retry_gas=4983183820790 storage_delta_failed=0 storage_delta_retry=0 round_advanced_once=true duplicate_retry_rejected=true
```

Gas figures include the initiating/retry transaction and their receipt chains as reported by near-workspaces. Storage deltas are the contract account's measured storage usage immediately before and after the corresponding receipt chain. No production contract code needed modification for this scenario; the change is in the Sandbox test and this documentation.

At the time of this 2026-11-13 verification, the transfer failure/retry path had been exercised with an actual optimized-Wasm Sandbox receipt failure. This local result did not establish Testnet deployment or Testnet integration.

## Release hardening phase 1 — creation amount validation — 2026-11-13

`Circle::open` now validates that `contribution_yocto × configured max_members` is representable as `u128` before accepting a circle. The configured membership target is used (2–10); the hard maximum is not substituted for the target. Overflow returns a distinct model error, and the create entrypoint reports `contribution amount times membership target overflows u128`. Zero contribution and invalid membership targets remain invalid configuration. Existing contribution and payout arithmetic is unchanged.

Added native boundary coverage for ordinary values, the largest valid contribution for target 2, the first overflowing contribution for target 2, the same amount accepted at target 2 and rejected at target 3, zero contribution, and target 1. Existing tests were preserved.

### Deadline interval policy — approved and implemented

`payment_interval_ns` is an unsigned 64-bit count of nanoseconds, configurable from 60 seconds through 365 days inclusive. The first deadline is computed from activation time, and each subsequent deadline begins after the previous payout succeeds. Timestamp additions use `checked_add`; payout initiation and retry preflight the next deadline so an overflow cannot occur after funds have been sent.

At the theoretical `u64` clock boundary, a successful payout is recorded and the circle enters `ClockExhausted`; the transfer is not left retryable. The native and Sandbox coverage for these paths is summarized in the current verification status and the release-hardening record below.

### Verification

Windows native checks:

```text
cargo fmt --manifest-path contract/Cargo.toml -- --check  PASS
cargo test --manifest-path contract/Cargo.toml --locked    24 passed, 0 failed
```

Optimized-Wasm local NEAR Sandbox check, run in Ubuntu under WSL2:

```text
cargo near build non-reproducible-wasm --locked --no-abi
WASM size=226379 bytes SHA-256=0b43fbce0df931e942d49c2eb8f1670964dcc41840989a1c2c378d1d51ef9b9e
AWS_LC_SYS_PREBUILT_NASM=0 cargo test --locked --features sandbox-tests --test sandbox -- --nocapture
1 passed, 0 failed; finished in 55.84s
SANDBOX_WASM_METRICS create_storage_bytes=4545 create_gas=6316571976508 ten_join_storage_bytes=2540 ten_join_gas=70594341712007 max_individual_join_gas=7522198659050 duplicate_rejected=true full_rejected=true max_capacity_payout_callback=true
SANDBOX_FAILED_TRANSFER_RETRY transfer_id=41 failed_attempt=1 retry_attempt=2 amount_yocto=2000 recipient_balance_delta=2000 failed_attempt_gas=5208983072835 retry_gas=4983436537230 storage_delta_failed=0 storage_delta_retry=0 round_advanced_once=true duplicate_retry_rejected=true
```

The Sandbox suite exercised creation, ten-member joins, financial lifecycle, an actual failed transfer receipt, and successful retry. The overflow boundaries are native model tests; Sandbox does not separately execute each `u128` creation boundary. No deadline interval policy or production change beyond creation-time amount validation was made in this phase. No frontend changes or Testnet deployment occurred.

## Release hardening phase 1 — deadline safety and lifecycle coverage — 2026-11-13

Implemented the approved payment interval range: 60 seconds through 365 days, stored as nanoseconds and configurable at circle creation. Values outside the inclusive range are rejected by the contract. The first activation deadline and later successful-payout deadlines continue to use checked timestamp addition.

Before a non-final payout is initiated or retried, the contract now checks that the next deadline can be represented at the current block timestamp. The callback also handles an extreme timestamp overflow without panicking: it records the successful transfer and finalized round, advances the round index once, and sets the circle to `ClockExhausted`. This avoids reverting callback state after a transfer has already succeeded, which would otherwise leave it retryable. At that theoretical `u64` clock boundary, the circle is terminal and its remaining collateral cannot be refunded under current rules; this is a safety freeze, not a normal recovery path. The normal deadline semantics remain activation time for round 1 and payout-success callback time for each subsequent round.

Default processing now emits an individual `default_collateral_applied` event for each newly substituted member, even when it then encounters an uncovered member and pauses. This avoids repeating prior substitution events when default processing resumes after a cure.

### New regression coverage

- Native interval tests cover the exact 60-second and 365-day endpoints, one unit below/above, and activation timestamp overflow without mutating membership.
- Native payout tests cover initiation and retry preflight at the timestamp boundary and a simulated successful transfer callback at `u64::MAX`; the latter verifies the transfer remains Succeeded, the round advances once, and the circle is `ClockExhausted`, never retryable.
- Native cancellation tests cover missing consent, revocation, duplicate approval/finalization, and cancellation attempts after a successful payout.
- A native accounting invariant helper checks direct and collateral-funded contribution totals, per-member contribution and payout totals, collateral conservation, transfer reservations, round payout state, and successful payout count across contribution, collateral substitution, failed transfer, retry, and success.
- Optimized-Wasm Sandbox now submits a deposit-refund claim asynchronously, deletes the member account before the transfer receipt, verifies the actual receipt failure and preserved 3,000 yoctoNEAR entitlement, recreates the account, retries the same operation, and verifies exactly one 3,000 yoctoNEAR balance increase.
- Optimized-Wasm Sandbox also verifies both collateral-applied and pause events when processing covers one missed member and then pauses on the next. The fixture advances 1,000 blocks to cross the approved minimum interval; it asserts the observed timestamp exceeds the deadline.

### Results

Windows native checks:

```text
cargo fmt --manifest-path contract/Cargo.toml -- --check  PASS
cargo test --manifest-path contract/Cargo.toml --locked    29 passed, 0 failed
```

Fresh optimized-Wasm build and local Sandbox run in Ubuntu under WSL2:

```text
cargo near build non-reproducible-wasm --locked --no-abi
WASM size=228159 bytes SHA-256=ff673d95967150fdef53c4251a20d42874182d061bf193d704e7e685a1cd86a0
AWS_LC_SYS_PREBUILT_NASM=0 cargo test --locked --features sandbox-tests --test sandbox -- --nocapture
1 passed, 0 failed; finished in 101.74s
SANDBOX_WASM_METRICS create_storage_bytes=4545 create_gas=6320789664936 ten_join_storage_bytes=2540 ten_join_gas=70634971815007 max_individual_join_gas=7526343122194 duplicate_rejected=true full_rejected=true max_capacity_payout_callback=true
SANDBOX_FAILED_TRANSFER_RETRY transfer_id=41 failed_attempt=1 retry_attempt=2 amount_yocto=2000 recipient_balance_delta=2000 failed_attempt_gas=5213366798654 retry_gas=4987805453441 storage_delta_failed=0 storage_delta_retry=0 round_advanced_once=true duplicate_retry_rejected=true
SANDBOX_FAILED_DEPOSIT_REFUND_RETRY transfer_id=47 failed_attempt=1 retry_attempt=2 amount_yocto=3000 recipient_balance_delta=3000 duplicate_retry_rejected=true
SANDBOX_COLLATERAL_EVENT partial_coverage_event=true subsequent_pause_event=true
```

The Sandbox receipt failures and retries are actual local NEAR runtime receipts. The `u64::MAX` callback edge is a native promise-result test, not a Sandbox timestamp simulation. A first Sandbox run with the old 100-block advance did not cross the newly approved 60-second deadline; that test-only advancement was raised to 1,000 blocks, and the final run passed the explicit deadline assertion. No Testnet deployment or frontend modification occurred.

## Frontend integration and release verification — 2026-11-13

The React application reads circle, member, round, storage-price, and per-member accounting data from contract view methods over the Testnet RPC. It submits contract mutations through a wallet configured explicitly for Testnet. Browser local storage contains only recently opened circle IDs and locally recorded transaction hashes/labels/timestamps; it does not store financial state. Supabase stores member chat and supplementary profiles only; see [supabase-chat.md](supabase-chat.md).

The production build is `npm run build` (TypeScript check followed by Vite build). Local development reads use the Vite proxy, with optional server-side `NEAR_RPC_URL` configured in ignored `frontend/.env.local`; absent that setting, it falls back to public Testnet RPC. `NEAR_RPC_URL` is never exposed as a Vite client variable. Production reads currently use public Testnet RPC directly. Contract account configuration uses `VITE_CONTRACT_ID`; optional chat configuration uses only the Supabase project URL and public anon key. See the root README, [testnet-deployment.md](testnet-deployment.md), and [supabase-chat.md](supabase-chat.md) for run and setup instructions.

## Supabase member chat — 2026-11-13

Added a Supabase migration, RLS policies, and three authenticated Edge Functions for one-use NEP-413 wallet challenges and Testnet contract membership checks. The browser chat panel loads prior messages, subscribes to circle-filtered Postgres Changes, displays profiles and connection state, and removes subscriptions when the circle changes. The database assigns sender identity from a server-verified NEAR account and applies content checks and a per-session send limit.

Historical verification snapshot (2026-11-13): the Supabase database suite and Sandbox run were unavailable in that environment, and hosted Supabase/Testnet wallet integration had not been exercised. Since then, the local Supabase CLI and Docker were installed and the local migration, 34 pgTAP tests, and anonymous REST smoke test passed. The optimized-WASM Sandbox test was rerun successfully on 2026-11-14. The owner later reports a live Testnet contract workflow for Circle #1; hosted Supabase chat and its live integration remain unverified.

Historical handover snapshot (2026-11-13): the native tests, frontend build, and optimized WASM build passed, producing the 228,215-byte artifact with SHA-256 `c05d249dd7dd6e6a2587ad2d0f420424479b1df78ef3cdbb8d5f75630871735b`. WSL access was unavailable at that time, so the Sandbox suite was not rerun in that session. The 2026-11-14 Sandbox pass is recorded above. No Testnet deployment occurred.

## Final deployment-preparation checks — 2026-11-13

Historical deployment-preparation snapshot (2026-11-13): native tests, optimized WASM compilation, frontend tests, and production build passed. At that time, the Sandbox result available in that session referred to an earlier 228,159-byte artifact; the 228,215-byte artifact had not yet been Sandbox-tested there. The exact artifact was subsequently exercised by the passing Sandbox test recorded above. No Testnet RPC state query, account creation, or deployment transaction was performed in that snapshot.
