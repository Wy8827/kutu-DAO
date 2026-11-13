import { errorResponse, handleOptions, jsonResponse, readJson, requireUser } from '../_shared/server.ts';

Deno.serve(async (request) => {
  const options = handleOptions(request);
  if (options) return options;
  try {
    const { user, admin } = await requireUser(request);
    const input = await readJson(request);
    const accountId = input?.accountId;
    if (typeof accountId !== 'string' || !/^[a-z0-9._-]{2,64}$/.test(accountId)) throw new Error('A valid NEAR account ID is required.');
    const recipient = Deno.env.get('NEAR_CHAT_AUTH_RECIPIENT');
    if (!recipient) throw new Error('NEAR chat authentication recipient is not configured.');
    const { data, error } = await admin.rpc('issue_near_chat_challenge', {
      p_auth_user_id: user.id,
      p_near_account_id: accountId,
      p_recipient: recipient,
    });
    if (error || !data?.[0]) throw error || new Error('Could not issue an authentication challenge.');
    const challenge = data[0];
    return jsonResponse({ challengeId: challenge.challenge_id, nonce: challenge.nonce_base64, message: challenge.message, recipient }, 200, request);
  } catch (error) {
    return errorResponse(error, 'Could not create wallet challenge.', request);
  }
});
