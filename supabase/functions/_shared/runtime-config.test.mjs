import test from 'node:test';
import assert from 'node:assert/strict';
import { corsHeadersForOrigin, resolveSupabaseKeys } from './runtime-config.mjs';

function legacyJwt(role) {
  const encode = (value) => Buffer.from(JSON.stringify(value)).toString('base64url');
  return `${encode({ alg: 'HS256', typ: 'JWT' })}.${encode({ role })}.signature`;
}

test('new Supabase key maps take precedence over valid legacy keys', () => {
  const keys = resolveSupabaseKeys({
    SUPABASE_PUBLISHABLE_KEYS: JSON.stringify({ default: 'sb_publishable_test_public_key_123456' }),
    SUPABASE_SECRET_KEYS: JSON.stringify({ default: 'sb_secret_test_private_key_123456' }),
    SUPABASE_ANON_KEY: legacyJwt('anon'),
    SUPABASE_SERVICE_ROLE_KEY: legacyJwt('service_role'),
  });
  assert.equal(keys.anonKey, 'sb_publishable_test_public_key_123456');
  assert.equal(keys.serviceKey, 'sb_secret_test_private_key_123456');
});

test('valid legacy anon and service-role JWT keys remain supported', () => {
  const keys = resolveSupabaseKeys({
    SUPABASE_ANON_KEY: legacyJwt('anon'),
    SUPABASE_SERVICE_ROLE_KEY: legacyJwt('service_role'),
  });
  assert.equal(keys.anonKey, legacyJwt('anon'));
  assert.equal(keys.serviceKey, legacyJwt('service_role'));
});

test('invalid configured modern keys fail instead of falling back to legacy values', () => {
  assert.throws(() => resolveSupabaseKeys({
    SUPABASE_PUBLISHABLE_KEYS: '{bad json',
    SUPABASE_SECRET_KEYS: JSON.stringify({ default: 'sb_secret_test_private_key_123456' }),
    SUPABASE_ANON_KEY: legacyJwt('anon'),
  }), /SUPABASE_PUBLISHABLE_KEYS must be a JSON object/);
  assert.throws(() => resolveSupabaseKeys({
    SUPABASE_PUBLISHABLE_KEYS: JSON.stringify({ default: 'sb_publishable_test_public_key_123456' }),
    SUPABASE_SECRET_KEYS: JSON.stringify({ default: 'sb_publishable_wrong_kind_123456' }),
    SUPABASE_SERVICE_ROLE_KEY: legacyJwt('service_role'),
  }), /SUPABASE_SECRET_KEYS\.default has an invalid key type/);
});

test('privileged key is required and legacy key role must be service_role', () => {
  assert.throws(() => resolveSupabaseKeys({
    SUPABASE_PUBLISHABLE_KEYS: JSON.stringify({ default: 'sb_publishable_test_public_key_123456' }),
    SUPABASE_SERVICE_ROLE_KEY: legacyJwt('anon'),
  }), /SUPABASE_SERVICE_ROLE_KEY has an invalid key type/);
  assert.throws(() => resolveSupabaseKeys({ SUPABASE_ANON_KEY: legacyJwt('anon') }), /Supabase secret key is not configured/);
});

test('CORS permits only exact explicitly configured origins', () => {
  const allowed = corsHeadersForOrigin('https://chat.example.test', 'https://chat.example.test');
  assert.equal(allowed['Access-Control-Allow-Origin'], 'https://chat.example.test');
  assert.equal(allowed['Access-Control-Allow-Methods'], 'POST, OPTIONS');
  assert.doesNotMatch(allowed['Access-Control-Allow-Origin'], /\*/);
  assert.equal(corsHeadersForOrigin('https://evil.example', 'https://chat.example.test'), null);
  assert.equal(corsHeadersForOrigin('https://chat.example.test/path', 'https://chat.example.test'), null);
});

test('localhost origins work only when explicitly listed and no-Origin calls get no allow-origin header', () => {
  assert.equal(corsHeadersForOrigin('http://localhost:5173', 'http://localhost:5173')['Access-Control-Allow-Origin'], 'http://localhost:5173');
  assert.equal(corsHeadersForOrigin('http://localhost:5173', '') , null);
  const noOrigin = corsHeadersForOrigin(null, '');
  assert.equal(noOrigin['Access-Control-Allow-Origin'], undefined);
  assert.equal(noOrigin['Access-Control-Allow-Methods'], 'POST, OPTIONS');
});
