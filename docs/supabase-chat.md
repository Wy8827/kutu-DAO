# Supabase circle chat setup

Supabase holds off-chain chat messages and optional display profiles only. Circle membership and every financial record remain on the NEAR Testnet contract. The browser never receives a service-role key.

## Create and configure a Supabase project

1. Create a Supabase project and keep its project URL, public anon/publishable key, and service-role key private to their intended locations.
2. In **Authentication → Providers → Anonymous**, enable anonymous sign-ins. The frontend creates an anonymous Supabase Auth session so Postgres can apply `authenticated` RLS policies before the user proves their NEAR identity.
3. Enable Realtime for database changes if the project dashboard requires an explicit publication/table selection. The migration adds `public.chat_messages` to `supabase_realtime` when that publication exists.
4. Install the Supabase CLI and Docker Desktop for local Supabase development, or use the Supabase SQL editor for migrations and deploy Edge Functions through the CLI/dashboard workflow.

## Apply the schema

The migration is [202611130001_circle_chat.sql](../supabase/migrations/202611130001_circle_chat.sql). It creates:

- `chat_messages`: circle-scoped content, verified sender, timestamps, and optional edit timestamp; content is limited to 1–1000 non-whitespace characters.
- `chat_profiles`: display names and optional HTTPS avatar URLs.
- `chat_identities`: a server-managed mapping from a Supabase Auth user UUID to a NEAR account.
- `circle_chat_memberships`: a server-managed cache of membership proven against the contract.
- Private one-time challenge records and per-user message rate-limit state.

RLS limits message select/insert/update to the signed-in user’s verified circle memberships. Profile reads are limited to the user’s own profile and currently verified peers in shared circles. A database trigger assigns the sender account from the verified identity, prevents edits to message identity fields, and enforces a 20-message-per-minute per-session limit. Authenticated clients cannot write identities or memberships. They can edit only their own message content and profile.

For local development, use `npx supabase start`, `npx supabase db reset`, and `npx supabase test db`. The SQL RLS regression suite is at [chat_rls.test.sql](../supabase/tests/database/chat_rls.test.sql). A clean local reset applies all migration files before tests run.

## Configure environment variables

Copy `frontend/.env.example` to `frontend/.env.local` and set:

- `VITE_CONTRACT_ID`: the deployed NEAR Testnet contract account.
- `VITE_SUPABASE_URL`: the project URL.
- `VITE_SUPABASE_ANON_KEY`: the project’s public anon/publishable key. RLS remains mandatory.

For local Edge Functions, copy `supabase/functions/.env.example` to `supabase/functions/.env` and configure its local Supabase values plus `NEAR_CONTRACT_ID`, `NEAR_CHAT_AUTH_RECIPIENT`, and `CHAT_ALLOWED_ORIGINS`. Do not copy hosted credentials into this file. Hosted Edge Functions receive `SUPABASE_URL` and the project's key maps from the Supabase runtime; the server helper prefers `SUPABASE_PUBLISHABLE_KEYS` and `SUPABASE_SECRET_KEYS`, with validated fallbacks to the legacy key variables. Do not set the reserved `SUPABASE_` variables as project secrets. Configure `NEAR_CONTRACT_ID`, `NEAR_CHAT_AUTH_RECIPIENT`, and `CHAT_ALLOWED_ORIGINS` for the hosted project's Edge Functions. The privileged secret key is only used server-side and must never be prefixed with `VITE_`, put in frontend files, or committed. Both local `.env` paths are ignored by Git.

The frontend `VITE_SUPABASE_ANON_KEY` value may contain the project's public anon key or publishable key. It is public client configuration and is not an authorization boundary; RLS remains required. The server helper selects the `default` key from the new runtime JSON maps when present. If a new variable is present but malformed or has the wrong key type, startup fails instead of falling back. Legacy anon and service-role JWTs remain supported and their role claims are checked. Missing or invalid privileged keys fail closed.

`CHAT_ALLOWED_ORIGINS` is a comma-separated list of exact web origins, with no path or trailing slash, for example `http://localhost:5173,http://127.0.0.1:5173` during local Vite development. Localhost is not implicitly trusted. Set the hosted value to the exact deployed frontend origin. Origins absent from the list receive HTTP 403 and no `Access-Control-Allow-Origin` header. Requests without an `Origin` header are allowed for non-browser clients, but still require valid JWT authentication. CORS is not an authorization mechanism.

`NEAR_CONTRACT_ID` must be the same verified `.testnet` contract account configured as frontend `VITE_CONTRACT_ID`. Membership and signing-key checks use the hard-coded NEAR Testnet RPC endpoint. `NEAR_CHAT_AUTH_RECIPIENT` is a separate, stable application identifier shown to wallets when they sign in; the current implementation uses `kutu-dao-chat`. It is not the contract account ID: the challenge stores this recipient and the verifier compares it with the same configured value. Keep it stable across challenge and verification functions.

## Hosted integration test procedure (future manual steps)

These commands are provided for a dedicated, disposable Supabase **test project** only. They were not run as part of this repository work. Do not use a production project. Confirm the project ref in the Dashboard before running any command that names it. Replace each `<placeholder>` with its real value before execution; do not type the angle brackets. The database push and function deploy commands modify the selected hosted test project.

1. In the Supabase Dashboard, create/select an isolated test project. Under **Authentication → Providers → Anonymous**, enable anonymous sign-ins; `[auth].enable_anonymous_sign_ins` in this repository configures only the local stack. Enable Realtime for `public.chat_messages` in the `supabase_realtime` publication if it is not already enabled. The migration adds the table conditionally when that publication exists.
2. Obtain the test project's project ref, project URL, and public anon/publishable key. Keep any project database password and CLI access token in your own secure local credential flow; do not put them in this repository or share them in chat.
3. Log the CLI into your own Supabase account interactively, then apply this repository's migration to the explicitly named test project:

   ```powershell
   npx supabase login
   npx supabase db push --remote=<test-project-ref>
   ```

   Review the target and migration prompt before confirming. Do not add `--yes`. This migration is [202611130001_circle_chat.sql](../supabase/migrations/202611130001_circle_chat.sql).

4. In the test project's **Edge Functions → Secrets**, set `NEAR_CONTRACT_ID` to the already deployed Testnet contract account, `NEAR_CHAT_AUTH_RECIPIENT` to `kutu-dao-chat`, and `CHAT_ALLOWED_ORIGINS` to the exact test frontend origin. For local Vite testing use only explicitly listed origins such as `http://localhost:5173` and `http://127.0.0.1:5173`. The Supabase URL and key maps are provided by the hosted runtime; do not paste service-role/secret keys into source files or commands. The helper prefers new key maps and validates legacy fallbacks.
5. Deploy only the three functions to the same explicit test project:

   ```powershell
   npx supabase functions deploy near-chat-challenge --project-ref <test-project-ref>
   npx supabase functions deploy near-chat-verify --project-ref <test-project-ref>
   npx supabase functions deploy authorize-circle-chat --project-ref <test-project-ref>
   ```

   `verify_jwt = true` is configured for all three in `supabase/config.toml`; retain JWT verification. Each caller must have a valid Supabase session JWT. The function code also resolves the user with Auth before using its privileged server client.

6. Copy `frontend/.env.example` to the ignored `frontend/.env.local` and fill in the test project's URL/public key plus the already deployed `.testnet` contract ID. Start the frontend with `npm run dev`; do not place a service-role/secret key in this file.
7. Use two Testnet wallets that have joined the same on-chain circle. Authenticate both, send messages in each direction, reload history, paginate, switch circles, disconnect/reconnect the network, and confirm duplicate delivery is suppressed. Use a third non-member wallet to check authorization denial. Confirm the test project's function logs contain no secret values.
8. The contract currently has no member-leave/removal operation, so on-chain membership revocation cannot be tested with the current contract. Cache expiry can be checked locally, but is not a substitute for a genuine Testnet revocation test. Do not claim revocation integration until a Testnet contract operation can actually remove a member and the cache expiry/realtime behavior is exercised.

The shared CORS helper returns headers only for explicitly configured origins and rejects unapproved requests before authentication or business logic. A local Supabase CLI 3.1.0 HTTP smoke test on 2026-11-13 found that the local request boundary adds wildcard CORS headers and handles preflight outside the function handler. The exact observations are recorded below. This means the local HTTP endpoint does not verify the allowlist at the browser boundary, even though the function code does. No hosted project was contacted; hosted behavior remains unverified. Supabase's hosted Edge Function guidance assigns preflight and response CORS headers to the function, and the self-hosted Envoy documentation says `/functions/v1/` leaves CORS to the function. Verify the hosted endpoint directly before treating the allowlist as effective there: [Edge Function CORS guide](https://supabase.com/docs/guides/functions/cors), [self-hosted gateway behavior](https://supabase.com/docs/guides/self-hosting/self-hosted-envoy).

### Local CORS HTTP observations

Requests were sent to the local `near-chat-challenge` endpoint using an anonymous local Auth session for authenticated cases. The Edge Runtime request log showed no function dispatch for either `OPTIONS`; actual authenticated requests reached the function. The missing-JWT rejection returned the function's allow-headers/methods, while an invalid JWT was rejected before function dispatch. No JWT or API key was printed.

| Case | HTTP | `Access-Control-Allow-Origin` | `Access-Control-Allow-Headers` | `Access-Control-Allow-Methods` | Function handler |
| --- | ---: | --- | --- | --- | --- |
| Approved origin, OPTIONS | 200 | `*` | `authorization,apikey,content-type,x-client-info` | `GET,HEAD,PUT,PATCH,POST,DELETE,OPTIONS,TRACE,CONNECT` | No; preflight answered before Edge Function dispatch |
| Unapproved origin, OPTIONS | 200 | `*` | `authorization,apikey,content-type,x-client-info` | `GET,HEAD,PUT,PATCH,POST,DELETE,OPTIONS,TRACE,CONNECT` | No; preflight answered before Edge Function dispatch |
| Approved origin, authenticated POST | 200 | `*` | `authorization, apikey, content-type, x-client-info` | `POST, OPTIONS` | Yes; challenge response returned |
| Unapproved origin, authenticated POST | 403 | `*` | absent | absent | Yes; shared origin guard returned 403 before auth/business logic |
| Missing Origin, authenticated POST | 200 | `*` | `authorization, apikey, content-type, x-client-info` | `POST, OPTIONS` | Yes; challenge response returned |
| Approved origin, missing JWT | 401 | `*` | `authorization, apikey, content-type, x-client-info` | `POST, OPTIONS` | Yes; function authentication rejected it |
| Approved origin, invalid JWT | 401 | `*` | absent | absent | No; rejected before Edge Function dispatch |

The 403 is not evidence that browser CORS is enforced: its wildcard header allows a browser to read the denial. The wildcard is absent from `_shared/runtime-config.mjs` and `_shared/server.ts`; the emitted preflight methods also differ from the helper's `POST, OPTIONS`. The requests went through the local API endpoint on port 54321 while the `supabase_kong` gateway container was running. Preflight bypassing the function and wildcard headers on the function's own 403/200 responses show that the local gateway layer answers `OPTIONS` and adds/overrides CORS headers; this does not come from the shared helper. Supabase tracks the same self-hosted gateway discrepancy in [issue #52346](https://github.com/supabase/supabase/issues/52346), which was closed with a fix, but the behavior was still present in this running local stack. The request observations identify the gateway layer; they do not distinguish the precise Kong plugin/configuration from any local gateway wrapper around it. This project did not change the function allowlist to accommodate that local behavior, because doing so would weaken the application policy. Existing Node regression tests exercise exact-origin matching, no-origin behavior, and denial of unapproved origins at the helper boundary; they cannot test a gateway that runs outside Node.

For the hosted test, first send `OPTIONS` with an approved origin and require that the response allows exactly that origin and the needed `POST` method/headers. Repeat with an unapproved origin and require no `Access-Control-Allow-Origin` (or a rejection) and no readable response. Then send authenticated POSTs from both origins: require the approved call to succeed and the disallowed call to return 403 without any allow-origin header. A 403 carrying `*` is still a failed CORS check. Also test missing/invalid JWT and confirm no data is returned. Use the browser Network panel or `curl.exe -i` against the exact hosted function URL; `curl` reports headers but does not enforce browser CORS. Do not put a session JWT in shell history, logs, or this repository. Hosted results must be recorded before claiming CORS verified.

Anonymous sign-ins are enabled without a CAPTCHA flow in this frontend, so use a private test project for integration. Before public exposure, integrate a CAPTCHA provider such as Cloudflare Turnstile or hCaptcha with anonymous sign-in and enable the matching Supabase Auth bot protection.

## Wallet authentication

1. The browser creates or resumes a Supabase anonymous Auth session.
2. `near-chat-challenge` creates a random 32-byte, five-minute, one-use challenge tied to that Supabase user. Challenge issuance is capped at 10 per five minutes per session.
3. The browser sends its selected NEAR account ID with the challenge request. The server embeds that account in the signed message; the wallet adapter signs it using NEP-413 `signMessage`, and the returned signing account must match.
4. `near-chat-verify` retrieves the challenge from the database, checks the public key is a current full-access key of that NEAR account on Testnet, and verifies the NEP-413 signature server-side. A database transaction consumes the challenge and binds the verified NEAR account to the Supabase Auth user.

The challenge uses a server-generated 32-byte nonce, fixed recipient, account-bound message, five-minute expiration, per-session issuance lock/rate limit, a five-attempt verification cap, and atomic one-time consumption to prevent replay and concurrent rate-limit bypass. Invalid signatures are rejected before any NEAR RPC call. The verifier currently supports Ed25519 NEP-413 signatures and fails closed for other key types. It checks that the exact signing key is currently a full-access key on Testnet immediately before binding; a key revoked before that check is rejected. No wallet private key or access key is collected. A Supabase session remains bound to one NEAR account; changing wallets creates a fresh anonymous session. A lost browser’s anonymous Supabase identity cannot be recovered; the user can create a new anonymous session and prove the same wallet again. Multiple anonymous sessions may map to the same NEAR account, which is expected; the durable profile is keyed by NEAR account, while each chat session has its own auth identity and rate-limit bucket.

## Circle membership authorization

Before loading a circle’s chat, the frontend calls `authorize-circle-chat` with the circle ID only. The Edge Function obtains the NEAR account from the server-managed identity mapping, calls `get_members` on the configured Testnet contract, and writes a membership cache row only if the account appears in that result. The contract returns permanent roster records. Its member statuses (`Joined`, `RefundPending`, `Refunded`) describe financial participation/refund handling; there is no inactive, suspended, or removed status and no member-removal operation. `RefundPending` and `Refunded` can occur after cancellation or completion and do not delete a member from the roster. The Edge Function does not query `get_circle` or filter by circle status, so historical roster members can continue accessing chats for completed or cancelled circles. This is the current policy, not an on-chain revocation feature. Unit tests cover exact identity matching and all three roster statuses. Any future rule to close chats on cancellation/completion needs explicit product policy and should not conflate refund state with removal.

The RLS membership helper matches both Supabase Auth UUID and verified NEAR account. The client cannot insert or alter those rows. `get_members` requires an existing circle ID and is a view method; contract initialization is `new()` with no arguments.

The membership cache expires after five minutes. The RLS helper enforces that expiry on direct REST reads, writes, and Realtime row delivery. An open chat refreshes membership from the Testnet contract every four minutes; failure closes chat access. A Testnet RPC outage fails closed and chat access is not granted. The short cache bounds stale authorization after an on-chain membership change; a message operation racing with an on-chain removal can still succeed until its current cache expires, so this is suitable for an MVP rather than immediate revocation.

## Realtime chat behavior

The chat loads the latest 50 messages, with a “Load earlier messages” control for history, then subscribes to `INSERT` events filtered to the current circle. The subscription is removed when the circle changes or the chat unmounts. Supabase Realtime applies the same `SELECT` RLS policy as history reads. The UI shows channel status, offline state, errors, and a reconnecting state; Supabase Realtime handles retries when connectivity returns.

## Test locally

1. Start local Supabase with `supabase start` and apply migrations with `supabase db reset`.
2. Enable anonymous sign-ins in the local Auth configuration if they are not enabled already.
3. Create or deploy the NEAR contract to Testnet and set its account ID in both frontend and Edge Function environment files. The Edge Function does not use a local/mock membership list.
4. Serve the functions using `supabase functions serve --env-file supabase/functions/.env` (or the CLI’s per-function serving command if required by your CLI version).
5. Run `npm run dev`, configure `frontend/.env.local`, connect a Testnet wallet that supports NEP-413 signing, and open a circle after joining it on-chain.
6. Open a second browser profile with another Testnet wallet that has joined the same circle. Send messages from both clients and confirm new messages appear without refresh. Open a circle ID where one account is not a member and confirm chat is denied.
7. Run `npm test` for frontend behavior and NEP-413 signature verification; run `supabase test db` for RLS/database tests.

Testing chat authentication and membership requires a deployed Testnet contract and live Supabase configuration. No such credentials are included in this repository, and no deployment is performed by these instructions.

## Security limitations

- Anonymous Auth identities are browser-local until the user proves a wallet again; they do not provide a recoverable cross-device profile login.
- The per-session chat and challenge rate limits can be bypassed by creating additional anonymous Auth users. Enable Supabase CAPTCHA/bot protection and review project-level Auth rate limits before a public launch; no paid infrastructure is required for the application-level controls.
- Rechecking key validity and membership is subject to a small time-of-check/time-of-use window because NEAR state can change immediately after the Testnet RPC response.
- Membership checks depend on the availability and correctness of the configured Testnet contract and RPC. The Supabase mirror is only an authorization cache, never financial state.
- This implementation has not received an independent security audit. Do not treat the chat integration as audited infrastructure.
