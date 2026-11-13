function legacyJwtRole(key) {
  const parts = key.split('.');
  if (parts.length !== 3 || !parts[1]) return null;
  try {
    const payload = parts[1].replace(/-/g, '+').replace(/_/g, '/');
    const decoded = atob(payload.padEnd(Math.ceil(payload.length / 4) * 4, '='));
    return JSON.parse(decoded).role ?? null;
  } catch {
    return null;
  }
}

function validateKey(key, type, source) {
  if (typeof key !== 'string' || key.length < 20) throw new Error(`${source} is missing or invalid.`);
  const newPrefix = type === 'publishable' ? 'sb_publishable_' : 'sb_secret_';
  const legacyRole = type === 'publishable' ? 'anon' : 'service_role';
  if (key.startsWith(newPrefix) || legacyJwtRole(key) === legacyRole) return key;
  throw new Error(`${source} has an invalid key type.`);
}

function fromKeyMap(raw, variable, type) {
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error(`${variable} must be a JSON object containing a default key.`);
  }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error(`${variable} must be a JSON object containing a default key.`);
  }
  return validateKey(parsed.default, type, `${variable}.default`);
}

function resolveKey(env, modernVariable, legacyVariable, type) {
  const modern = env[modernVariable];
  if (modern !== undefined && modern !== '') return fromKeyMap(modern, modernVariable, type);
  const legacy = env[legacyVariable];
  if (legacy !== undefined && legacy !== '') return validateKey(legacy, type, legacyVariable);
  throw new Error(`Supabase ${type} key is not configured (${modernVariable} or ${legacyVariable}).`);
}

export function resolveSupabaseKeys(env) {
  return {
    anonKey: resolveKey(env, 'SUPABASE_PUBLISHABLE_KEYS', 'SUPABASE_ANON_KEY', 'publishable'),
    serviceKey: resolveKey(env, 'SUPABASE_SECRET_KEYS', 'SUPABASE_SERVICE_ROLE_KEY', 'secret'),
  };
}

function normalizeOrigin(value) {
  try {
    const url = new URL(value);
    if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) return null;
    if (url.origin !== value || url.pathname !== '/' || url.search || url.hash) return null;
    return url.origin;
  } catch {
    return null;
  }
}

export function corsHeadersForOrigin(origin, configuredOrigins) {
  const headers = {
    'Access-Control-Allow-Headers': 'authorization, apikey, content-type, x-client-info',
    'Access-Control-Allow-Methods': 'POST, OPTIONS',
    'Vary': 'Origin',
  };
  if (!origin) return headers;
  const normalizedOrigin = normalizeOrigin(origin);
  const allowlist = String(configuredOrigins ?? '').split(',').map((item) => normalizeOrigin(item.trim())).filter(Boolean);
  if (!normalizedOrigin || !allowlist.includes(normalizedOrigin)) return null;
  return { ...headers, 'Access-Control-Allow-Origin': normalizedOrigin };
}
