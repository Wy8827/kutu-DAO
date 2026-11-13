import { verifyNep413Signature } from '../_shared/near-auth.mjs';
import { hasFullAccessKey } from '../_shared/near-rpc.ts';
import { errorResponse, handleOptions, jsonResponse, readJson, requireUser } from '../_shared/server.ts';

Deno.serve(async (request) => {
  const options = handleOptions(request);
  if (options) return options;
  try {
    const { user, admin } = await requireUser(request);
    const input = await readJson(request);
    const challengeId = input.challengeId;
    const accountId = input.accountId;
    const publicKey = input.publicKey;
    const signature = input.signature;
    if (typeof challengeId !== 'string' || typeof accountId !== 'string' || typeof publicKey !== 'string' || typeof signature !== 'string') {
      throw new Error('Signed challenge fields are missing.');
    }
    if (!/^[a-z0-9._-]{2,64}$/.test(accountId) || !publicKey.startsWith('ed25519:')) throw new Error('Unsupported NEAR account or signing key.');
    const { data, error } = await admin.rpc('get_near_chat_challenge', { p_challenge_id: challengeId, p_auth_user_id: user.id });
    if (error || !data?.[0]) throw new Error('Challenge expired or already used. Request a new one.');
    const challenge = data[0];
    const configuredRecipient = Deno.env.get('NEAR_CHAT_AUTH_RECIPIENT');
    if (!configuredRecipient || challenge.recipient !== configuredRecipient) throw new Error('Authentication recipient mismatch.');
    const valid = await verifyNep413Signature({
      message: challenge.message,
      nonceBase64: challenge.nonce_base64,
      recipient: challenge.recipient,
      publicKey,
      signatureBase64: signature,
    });
    if (!valid) throw new Error('NEAR wallet signature is invalid.');
    if (!await hasFullAccessKey(accountId, publicKey)) throw new Error('The signing key is not a full-access key for this NEAR Testnet account.');
    const { data: boundAccount, error: bindError } = await admin.rpc('complete_near_chat_auth', {
      p_challenge_id: challengeId,
      p_auth_user_id: user.id,
      p_near_account_id: accountId,
      p_recipient: challenge.recipient,
    });
    if (bindError || boundAccount !== accountId) throw bindError || new Error('Could not bind verified wallet identity.');
    return jsonResponse({ accountId }, 200, request);
  } catch (error) {
    return errorResponse(error, 'Could not verify NEAR wallet signature.', request);
  }
});
