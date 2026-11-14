import test from 'node:test';
import assert from 'node:assert/strict';
import { formatYoctoNearExact, payoutHistory, payoutParentTransaction } from '../src/payout-history.ts';

test('payout history includes only started payouts and preserves all final states in round order', () => {
  const rounds = [
    { number: 2, payout: { id: 3, recipient_id: 'c.testnet', amount_yocto: '300', state: 'Failed' } },
    { number: 0, payout: { id: 1, recipient_id: 'a.testnet', amount_yocto: '100', state: 'Succeeded' } },
    { number: 1, payout: { id: 2, recipient_id: 'b.testnet', amount_yocto: '200', state: 'Pending' } },
    { number: 3, payout: { id: 0, recipient_id: 'd.testnet', amount_yocto: '400', state: 'NotStarted' } },
  ];

  assert.deepEqual(payoutHistory(rounds).map((round) => [round.number, round.payout.state]), [
    [0, 'Succeeded'], [1, 'Pending'], [2, 'Failed'],
  ]);
});

test('payout parent transaction lookup matches circle and round metadata only', () => {
  const activity = [
    { hash: 'parent-1', label: 'Finalize payout', time: 10, circleId: 1, roundNumber: 0, payout: true },
    { hash: 'parent-2', label: 'Finalize payout', time: 20, circleId: 2, roundNumber: 0, payout: true },
    { hash: 'unlinked', label: 'Finalize payout', time: 30 },
  ];

  assert.equal(payoutParentTransaction(activity, 1, 0)?.hash, 'parent-1');
  assert.equal(payoutParentTransaction(activity, 1, 1), undefined);
});

test('yoctoNEAR payout amounts are formatted without truncating fractional precision', () => {
  assert.equal(formatYoctoNearExact('1000000000000000000000000'), '1 NEAR');
  assert.equal(formatYoctoNearExact('123456789012345678901234'), '0.123456789012345678901234 NEAR');
  assert.equal(formatYoctoNearExact('1'), '0.000000000000000000000001 NEAR');
});
