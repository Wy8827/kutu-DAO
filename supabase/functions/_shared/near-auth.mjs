const NEP413_TAG = 2 ** 31 + 413;

function concatBytes(...parts) {
  const output = new Uint8Array(parts.reduce((length, part) => length + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    output.set(part, offset);
    offset += part.length;
  }
  return output;
}

function u32(value) {
  const bytes = new Uint8Array(4);
  new DataView(bytes.buffer).setUint32(0, value, true);
  return bytes;
}

function borshString(value) {
  const bytes = new TextEncoder().encode(value);
  return concatBytes(u32(bytes.length), bytes);
}

function decodeBase58(value) {
  const alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  let number = 0n;
  for (const character of value) {
    const digit = alphabet.indexOf(character);
    if (digit < 0) throw new Error('Invalid base58 data');
    number = number * 58n + BigInt(digit);
  }
  const bytes = [];
  while (number > 0n) {
    bytes.unshift(Number(number & 255n));
    number >>= 8n;
  }
  for (const character of value) {
    if (character !== '1') break;
    bytes.unshift(0);
  }
  return Uint8Array.from(bytes);
}

function decodeBase64(value) {
  if (typeof value !== 'string' || !/^[A-Za-z0-9+/]+={0,2}$/.test(value)) throw new Error('Invalid base64 data');
  const binary = atob(value);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

export function encodeNep413Payload({ message, nonce, recipient }) {
  if (typeof message !== 'string' || typeof recipient !== 'string') throw new TypeError('Invalid NEP-413 payload');
  const nonceBytes = typeof nonce === 'string' ? decodeBase64(nonce) : nonce;
  if (!(nonceBytes instanceof Uint8Array) || nonceBytes.length !== 32) throw new TypeError('NEP-413 nonce must be 32 bytes');
  return concatBytes(u32(NEP413_TAG), borshString(message), nonceBytes, borshString(recipient));
}

export async function verifyNep413Signature({ message, nonceBase64, recipient, publicKey, signatureBase64 }) {
  try {
    const [keyType, encodedKey, extraKeyPart] = String(publicKey).split(':');
    if (keyType !== 'ed25519' || !encodedKey || extraKeyPart) return false;
    const keyBytes = decodeBase58(encodedKey);
    const signature = decodeBase64(signatureBase64);
    if (keyBytes.length !== 32 || signature.length !== 64) return false;
    const payload = encodeNep413Payload({ message, nonce: nonceBase64, recipient });
    const digest = await crypto.subtle.digest('SHA-256', payload);
    const key = await crypto.subtle.importKey('raw', keyBytes, { name: 'Ed25519' }, false, ['verify']);
    return await crypto.subtle.verify({ name: 'Ed25519' }, key, signature, digest);
  } catch {
    return false;
  }
}
