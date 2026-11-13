import test from 'node:test';
import assert from 'node:assert/strict';
import { contractRosterIncludesAccount } from './membership.mjs';

test('chat membership requires an exact account match in the contract roster', () => {
  assert.equal(contractRosterIncludesAccount([{ account_id: 'alice.testnet' }], 'alice.testnet'), true);
  assert.equal(contractRosterIncludesAccount([{ account_id: 'alice.testnet' }], 'alice2.testnet'), false);
  assert.equal(contractRosterIncludesAccount([], 'alice.testnet'), false);
  assert.equal(contractRosterIncludesAccount(null, 'alice.testnet'), false);
});

test('refund lifecycle statuses do not represent contract roster revocation', () => {
  for (const status of ['Joined', 'RefundPending', 'Refunded']) {
    assert.equal(
      contractRosterIncludesAccount([{ account_id: 'alice.testnet', status }], 'alice.testnet'),
      true,
      `${status} preserves the permanent roster record`,
    );
  }
});
