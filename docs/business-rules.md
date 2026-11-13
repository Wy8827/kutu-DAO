# Kutu DAO business rules

These are the MVP rules enforced by the contract. The frontend can explain and preview them but is not authoritative.

## Circle and membership

1. A circle supports a configured target of 2–10 members; the hard contract maximum is 10. Name is at most 64 UTF-8 bytes and description at most 280 UTF-8 bytes.
2. The creator is recorded separately and must explicitly join. Anyone may join an Open circle with available slots by attaching the exact security collateral and separate storage funding. Duplicate members, full circles, and joins after activation are rejected.
3. Each successful join gets the next permanent one-based payout position. Failed joins consume no position. The final successful join activates the circle. The member list, order, and circle financial rules are immutable thereafter.
4. The first contribution window begins at activation, not creation. Its length is the configured payment interval. A later round's deadline begins only after the preceding payout succeeds, giving each round a complete interval after the preceding transfer. Contributions are accepted at `now == deadline`; default processing requires `now > deadline`.

## Contributions, default and recovery

1. Each member contributes exactly the configured amount once per round. The contract derives the member from the signer and the round from current state. No recipient or member position is supplied by the caller.
2. Anyone may call default processing after the deadline. The contract scans missing positions in immutable join order. For each missing position, it either deducts exactly one contribution from that same member's collateral and records a collateral substitution, or, when collateral is insufficient, deducts nothing, records `AwaitingCure`, and pauses at that first unresolved member.
3. While Paused, only the recorded defaulted member can cure by attaching exactly one outstanding contribution. That new payment is recorded in the direct-payment slot with `Cured` resolution. It is not a collateral deduction. Wrong-account, short, excess, duplicate, or wrong-state cures fail atomically.
4. After cure, the circle resumes and ordered default processing continues for remaining unpaid positions. The round becomes ready only when every member slot is directly paid or fully covered by eligible collateral.
5. If an unresolved default remains, the circle stays Paused indefinitely. There are no new rounds, payouts, automatic refunds, redistribution, or administrator withdrawals while paused.

## Payouts and transfer retries

1. Any account can finalize a fully funded round. The recipient is fixed by payout position `round number + 1`; the amount is exactly `target member count × contribution`.
2. The contract records/reserves a transfer ID, original recipient, amount, and attempt before scheduling the asynchronous native transfer. Only a private callback can settle it; the callback validates caller, one result, ID, attempt, and Pending state.
3. Success finalizes the round once and opens the next round's deadline from the success block, or completes the final round. Failure preserves the original entitlement and marks the transfer Failed. Anyone can retry only that failed operation; retries keep its ID, recipient, and amount and increment the attempt. Duplicate retries and duplicate/stale callbacks fail.
4. No circle administrator or caller can redirect funds or choose the amount. There is no arbitrary withdrawal method.

## Refunds and unanimous cancellation

1. Cancellation is available only before any payout has succeeded, and only with explicit on-chain approval from every current member. An approval can be revoked before finalization. Finalization is permissionless after unanimity and only when no payout transfer is pending.
2. At finalization, each entitlement is frozen from that member's remaining collateral + direct contributions still held + collateral substitutions still held. A substitution has already reduced remaining collateral and is counted only in the round contribution ledger. Occupied-storage cost and unrelated storage allowance transfers are excluded.
3. Each refund claim goes to the immutable member account at the saved position. Claiming moves the frozen entitlement into its fixed transfer slot. Success settles it; failure preserves that same amount for retry and cannot be claimed again as a new operation.
4. If even one member never approves, cancellation cannot finalize. No party can force consent or withdraw/redirect another member's funds. Circle remains Active or Paused as-is and liabilities stay recorded.
5. After all scheduled payouts succeed, each member may permissionlessly claim their own remaining collateral to their own account. The refund is reserved before transfer and failure remains retryable. Contributions have already been paid out and are not part of this refund.
6. A creator may close a memberless Open circle. This does not permit the creator to claim the storage cost while its records remain.

## Storage funding and funds separation

1. Storage is an additional payment, separate from member collateral and contributions. Contract measures actual flushed storage bytes and multiplies by the current NEAR storage byte cost. Join funding must cover exact collateral plus measured storage cost; creation must cover measured storage cost. Insufficient attached funds revert the operation.
2. Any overpayment above measured storage cost is returned asynchronously to the payer through a fixed-recipient slot. A failed transfer remains retryable. The storage cost itself is not promised as a full refund while records occupy storage.
3. Collateral substitutions are an internal reclassification: collateral liability decreases by `q`, and the same member's round contribution record increases by `q`. They are never double counted.
4. Pending transfers are unavailable to other operations. All money values use checked integer yoctoNEAR amounts; views encode them as decimal strings.

## Contract statuses

- Circle: `Open`, `Active`, `Paused`, `Completed`, `Cancelled`.
- Round: `Collecting`, `ReadyForPayout`, `PayoutPending`, `TransferFailed`, `Finalized`.
- Default resolution per member/round: `NotProcessed`, `AwaitingCure`, `Cured`, `CollateralSubstituted`.
- Transfer: `NotStarted`, `Pending`, `Succeeded`, `Failed`.

Implementation and verification status is maintained in [development.md](development.md) and [financial-lifecycle.md](financial-lifecycle.md).
