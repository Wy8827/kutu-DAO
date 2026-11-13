import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash, generateKeyPairSync, sign } from 'node:crypto';
import { encodeNep413Payload, verifyNep413Signature } from './near-auth.mjs';

const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
function base58(bytes) {
  let value = BigInt(`0x${Buffer.from(bytes).toString('hex')}`);
  let output = '';
  while (value > 0n) { const digit = Number(value % 58n); output = alphabet[digit] + output; value /= 58n; }
  for (const byte of bytes) { if (byte !== 0) break; output = `1${output}`; }
  return output || '1';
}

test('NEP-413 verifier accepts a valid wallet signature and rejects tampering', async () => {
  const { publicKey: publicKeyObject, privateKey } = generateKeyPairSync('ed25519');
  const publicBytes = publicKeyObject.export({ format: 'der', type: 'spki' }).subarray(-32);
  const payload = { message: 'Kutu DAO chat sign-in\nChallenge: one-time', nonce: new Uint8Array(32).fill(7), recipient: 'kutu-dao-chat' };
  const digest = createHash('sha256').update(encodeNep413Payload(payload)).digest();
  const signature = sign(null, digest, privateKey).toString('base64');
  const input = { message: payload.message, nonceBase64: Buffer.from(payload.nonce).toString('base64'), recipient: payload.recipient, publicKey: `ed25519:${base58(publicBytes)}`, signatureBase64: signature };
  assert.equal(await verifyNep413Signature(input), true);
  assert.equal(await verifyNep413Signature({ ...input, recipient: 'other-app' }), false);
  assert.equal(await verifyNep413Signature({ ...input, message: `${payload.message}!` }), false);
  assert.equal(await verifyNep413Signature({ ...input, nonceBase64: 'bad' }), false);
  assert.equal(await verifyNep413Signature({ ...input, signatureBase64: Buffer.alloc(64).toString('base64') }), false);
});
