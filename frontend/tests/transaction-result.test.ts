import test from 'node:test';
import assert from 'node:assert/strict';
import { circleIdFromSuccessValue, circlesCreatedBy, getSuccessValueBase64 } from '../src/transaction-result.ts';

function toBase64(value: string) {
  return btoa(Array.from(new TextEncoder().encode(value), (byte) => String.fromCharCode(byte)).join(''));
}

test('extracts and decodes a circle ID from the wallet FinalExecutionOutcome once', () => {
  const encodedReturnValue = toBase64('2');
  const returnValue = getSuccessValueBase64({ SuccessValue: encodedReturnValue });
  assert.equal(returnValue, encodedReturnValue);
  assert.equal(circleIdFromSuccessValue(returnValue), 2);
});

test('distinguishes failed and incomplete wallet transaction outcomes', () => {
  assert.throws(() => getSuccessValueBase64({ Failure: { ActionError: 'rejected' } }), /transaction failed/i);
  assert.throws(() => getSuccessValueBase64({ SuccessReceiptId: 'receipt-id' }), /unrecognized or incomplete/);
  assert.throws(() => getSuccessValueBase64({ Unknown: null }), /unrecognized or incomplete/);
});

test('rejects malformed or unsafe circle return values instead of guessing an ID', () => {
  assert.throws(() => circleIdFromSuccessValue('2'), /base64|JSON|circle ID/i);
  assert.throws(() => circleIdFromSuccessValue(toBase64('0')), /outside the range/i);
  assert.throws(() => circleIdFromSuccessValue(toBase64('9007199254740992')), /invalid circle ID|outside the range/i);
});

test('creator circle results are account-filtered, deduplicated, and sorted', () => {
  const circles = [
    { id: 3, creator: 'alice.testnet', name: 'Three' },
    { id: 2, creator: 'alice.testnet', name: 'Two' },
    { id: 2, creator: 'alice.testnet', name: 'Duplicate' },
    { id: 1, creator: 'bob.testnet', name: 'Other wallet' },
  ];
  assert.deepEqual(circlesCreatedBy(circles, 'alice.testnet'), [circles[2], circles[0]]);
  assert.deepEqual(circlesCreatedBy(circles, ''), []);
});
