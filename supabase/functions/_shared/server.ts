import { createClient, type User } from 'npm:@supabase/supabase-js@2';
import { corsHeadersForOrigin, resolveSupabaseKeys } from './runtime-config.mjs';

function requestCorsHeaders(request?: Request) {
  return corsHeadersForOrigin(request?.headers.get('Origin') ?? null, Deno.env.get('CHAT_ALLOWED_ORIGINS'));
}

export function jsonResponse(body: unknown, status = 200, request?: Request) {
  const corsHeaders = requestCorsHeaders(request);
  if (!corsHeaders) return new Response('Origin not allowed', { status: 403 });
  return new Response(JSON.stringify(body), {
    status,
    headers: { ...corsHeaders, 'Content-Type': 'application/json' },
  });
}

export function handleOptions(request: Request) {
  const corsHeaders = requestCorsHeaders(request);
  if (!corsHeaders) return new Response('Origin not allowed', { status: 403 });
  if (request.method === 'OPTIONS') return new Response('ok', { headers: corsHeaders });
  if (request.method !== 'POST') return jsonResponse({ error: 'Method not allowed' }, 405, request);
  return null;
}

export function getSupabaseClients() {
  const url = Deno.env.get('SUPABASE_URL');
  if (!url) throw new Error('SUPABASE_URL is not configured.');
  const { anonKey, serviceKey } = resolveSupabaseKeys({
    SUPABASE_PUBLISHABLE_KEYS: Deno.env.get('SUPABASE_PUBLISHABLE_KEYS'),
    SUPABASE_SECRET_KEYS: Deno.env.get('SUPABASE_SECRET_KEYS'),
    SUPABASE_ANON_KEY: Deno.env.get('SUPABASE_ANON_KEY'),
    SUPABASE_SERVICE_ROLE_KEY: Deno.env.get('SUPABASE_SERVICE_ROLE_KEY'),
  });
  return {
    authClient: createClient(url, anonKey, { auth: { persistSession: false, autoRefreshToken: false } }),
    admin: createClient(url, serviceKey, { auth: { persistSession: false, autoRefreshToken: false } }),
  };
}

export async function requireUser(request: Request): Promise<{ user: User; admin: ReturnType<typeof getSupabaseClients>['admin'] }> {
  const token = request.headers.get('Authorization')?.replace(/^Bearer\s+/i, '');
  if (!token) throw new Error('A Supabase session is required.');
  const { authClient, admin } = getSupabaseClients();
  const { data, error } = await authClient.auth.getUser(token);
  if (error || !data.user) throw new Error('Supabase session is invalid or expired.');
  return { user: data.user, admin };
}

export async function readJson(request: Request): Promise<Record<string, unknown>> {
  const length = Number(request.headers.get('Content-Length') || 0);
  if (length > 8192) throw new Error('Request body is too large.');
  const body = await request.json();
  if (!body || typeof body !== 'object' || Array.isArray(body)) throw new Error('Expected a JSON object.');
  return body as Record<string, unknown>;
}

export function errorResponse(error: unknown, fallback = 'Request failed.', request?: Request) {
  const message = error instanceof Error ? error.message : fallback;
  const status = /not a member/i.test(message) ? 403
    : /session is required|session is invalid|unauthorized/i.test(message) ? 401
    : /rate limit|too many/i.test(message) ? 429
    : /not configured|temporarily unavailable/i.test(message) ? 503 : 400;
  return jsonResponse({ error: message || fallback }, status, request);
}
