# Kutu DAO architecture

**Status:** lifecycle policy implemented in the Rust contract; native Rust tests and optimized-Wasm Sandbox verification are recorded in `docs/development.md`. The owner reports version 0.1.0 deployed and initialized on NEAR Testnet at `kutunear.testnet`; see `docs/testnet-deployment.md` for the deployment record and read-only verification steps. This is a Testnet-only MVP design.

## System boundary

- The `near-sdk-rs` contract is the sole source of truth for circle rules, members, contributions, collateral, transfers, deadlines, payouts, cancellation, refunds, and storage charges.
- React + TypeScript + Tailwind reads contract views and requests wallet-signed calls. It does not calculate authoritative balances. Recent circle IDs and transaction links are browser convenience data only; the financial state comes from the contract.
- NEAR native transfer promises are asynchronous. Every outgoing transfer has a preallocated slot with a fixed recipient reference and amount, operation ID, attempt count, and state.
- Supabase stores only chat messages and supplementary profiles. It cannot decide circle state, membership, contributions, payouts, refunds, or balances. Circle membership for chat is granted by an Edge Function only after a Testnet contract view confirms the authenticated wallet is a member.
- Excluded from this MVP: DAO frameworks, stablecoins, meta-transactions for financial operations, AI agents, bridges, Intents, and Mainnet.

## Contract state (`contract/src/model.rs`)

Amounts use checked `u128` yoctoNEAR. View JSON serializes money and nanosecond timestamps as decimal strings to avoid JavaScript precision loss. Member slots are one-based and immutable once assigned. `MAX_MEMBERS` is 10; the configured target is 2–10.

### Circle

`Circle` stores the creator, immutable name/description/target/contribution/collateral/interval, activation timestamp, `CircleStatus`, successful-join-ordered `Vec<Member>`, preallocated `Vec<Round>`, separate measured storage bytes/cost, creator storage-excess transfer slot, current round, paused round/member position, and successful payout count. Circle states: `Open`, `Active`, `Paused`, `Completed`, `Cancelled`.

The creator is not implicitly enrolled. On the target-reaching successful join, the contract atomically activates the circle and starts round 0's deadline at `activation timestamp + payment interval`.

### Member

`Member` stores `account_id`, immutable `payout_position`, remaining refundable collateral, member storage byte/cost history, cumulative direct and substituted contributions, cumulative payouts and payment counters, member status, cancellation consent and frozen refund entitlement, plus preallocated `deposit_refund`, `cancellation_refund`, and `storage_allowance_refund` transfer slots.

### Round

`Round` stores zero-based number, predetermined recipient position (`number + 1`), opened/deadline timestamps, `RoundStatus`, fixed-size per-position vectors for direct payments and collateral substitutions, per-position `DefaultResolution`, total collected amount, payout transfer slot, and fixed-width finalized timestamp. Direct cure payments use the direct-payment vector and `Cured` resolution; collateral cover uses the substitutions vector and `CollateralSubstituted` resolution. Round states: `Collecting`, `ReadyForPayout`, `PayoutPending`, `TransferFailed`, `Finalized`.

Each later round opens only when the preceding payout callback succeeds; its deadline is that block timestamp plus the configured interval. `process_defaults` is permissionless after the strict deadline boundary (`now > deadline`).

### PendingTransfer

`PendingTransfer` stores globally unique `id`, `TransferKind`, immutable `TransferRecipient` (`Creator` or `MemberPosition`), immutable `amount_yocto`, `TransferState` (`NotStarted`, `Pending`, `Succeeded`, `Failed`), and attempt number. A retry requires Failed and the original ID, reuses the saved recipient/amount, increments attempt, and rejects old or duplicate callbacks. Callback authority is restricted to the contract itself and one promise result.

### Root state and storage version

`Contract` stores `schema_version`, monotonic circle/transfer IDs, aggregate pending outgoing and storage-funded amounts, and `near_sdk::store::LookupMap<u64, Circle>`. Stored Borsh structures/enums are version-sensitive. There is no migration entrypoint in this MVP; deploy a fresh account for the current schema. Any future migration needs an explicit versioned migration and compatibility tests before upgrade.

## Entry points and read methods

Mutations implemented in `contract/src/lib.rs`:

- `create_circle(args)`, `join_circle(circle_id)` — payable, storage cost separate from collateral.
- `contribute(circle_id)` — member-only by predecessor, exact configured amount, once for current round and by deadline.
- `process_defaults(circle_id)` — permissionless after deadline; full collateral substitution in position order or pause at the first insufficient balance without partial deduction.
- `cure_default(circle_id)` — paused member only, exact amount, direct payment ledger; re-runs ordered default processing after the cure.
- `finalize_payout(circle_id)` — permissionless when fully funded; recipient/amount derived entirely from contract rules.
- `approve_cancellation`, `revoke_cancellation`, `finalize_cancellation`, `close_empty_circle` — consent and pre-payout cancellation; only the creator can close a memberless Open circle.
- `claim_cancellation_refund`, `claim_deposit_refund`, `retry_transfer` — fixed-recipient refund claims and permissionless retry from Failed.
- `on_transfer_result` — private callback; validates one result, saved ID, current attempt, and Pending state.

Views: `get_storage_price_yocto`, `get_circle`, `get_members`, `get_round`, `get_member_accounting`, `get_transfer`. Events use NEP-297-style `EVENT_JSON` logs.

## Accounting model

- **Collateral liability:** each member's `collateral_remaining_yocto`; reduced only by an exactly recorded full contribution substitution or by reserving a refund. A reserved refund is represented only by its transfer slot.
- **Direct contribution escrow:** exact member payment, including a cure, attributed to the direct-payment slot for that round.
- **Collateral-funded contribution:** equal decrease in that member's remaining collateral and increase in that member's round substitution slot; never counted in both places.
- **Unpaid round escrow:** direct + substituted amounts until the payout succeeds or pre-payout cancellation refunds them.
- **Pending outgoing:** exact amount reserved in transfer slot and aggregate before promise scheduling. A failed payout remains a retryable round entitlement; failed refunds retain their fixed claim in the Failed slot.
- **Storage:** measured byte cost funded separately at create/join and tracked apart from all member financial liabilities. Excess attached storage allowance is a distinct fixed-recipient refund. Occupied storage cost is not promised back.

At unanimous cancellation, each member's frozen refund is `remaining collateral + their direct contributions still held + their collateral substitutions still held`. A collateral substitution is not counted twice: the original collateral amount has already been reduced. Storage fees are excluded. Pending storage-excess returns are separate claims.

For normal payout, round total must equal `target_members × contribution`. The pending recipient is the scheduled member position. Callback success settles the transfer once and opens the next round or completes the circle. Failure moves the round to `TransferFailed`; retry restores Pending with the same ID/recipient/amount. There is no caller-controlled recipient, amount, admin withdrawal, or arbitrary refund destination.

## Storage funding

For each payable create/join, the contract measures flushed storage growth (`after - before`) and multiplies it by `env::storage_byte_cost()` with checked arithmetic. Creation requires at least measured cost. Join requires the exact configured collateral plus measured storage cost. Underfunding reverts atomically. Excess is returned asynchronously. Round/member slots are preallocated; lifecycle calls check for storage growth where applicable. The frontend must explain the separate storage allowance and that occupied-state storage cost is not fully refundable.

The initial deployed contract footprint is outside circle accounting and must be funded by deployment endowment. The contract must never use member collateral or contributions to cover storage.

## Remaining limitations and risk

- No pruning/archive is implemented, so records and their associated storage cost remain on chain.
- No administrator-controlled recovery exists. If a member never consents to cancellation, a pre-payout circle cannot cancel and retains its recorded liabilities; an unresolved default remains Paused indefinitely.
- Asynchronous native transfers may fail if recipient account state disallows receiving. Saved entitlements remain retryable, but eventual success depends on recipient account availability and chain operation.
- Sandbox measurements are local runtime results, not Testnet receipts or a formal audit.
- The owner reports a live Testnet demonstration in which Circle #1 reached `Completed` after two members completed two contribution rounds. This reported state is not independently re-queried by this architecture document. No formal security audit is claimed.
- No in-contract admin upgrade or migration entrypoint is implemented. Schema changes require a reviewed migration plan; redeploying to the existing account can affect its deployed state and must not be done casually.

See [business-rules.md](business-rules.md), [financial-lifecycle.md](financial-lifecycle.md), and [development.md](development.md).
