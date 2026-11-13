import { testnetView } from '../_shared/near-rpc.ts';
import { contractRosterIncludesAccount } from '../_shared/membership.mjs';
import { errorResponse, handleOptions, jsonResponse, readJson, requireUser } from '../_shared/server.ts';

type Member = { account_id: string };

Deno.serve(async (request) => {
  const options = handleOptions(request);
  if (options) return options;
  try {
    const { user, admin } = await requireUser(request);
    const input = await readJson(request);
    const rawCircleId = input.circleId;
    if (typeof rawCircleId !== 'string' || !/^\d{1,16}$/.test(rawCircleId)) throw new Error('Invalid circle ID.');
    const circleNumber = Number(rawCircleId);
    if (!Number.isSafeInteger(circleNumber)) throw new Error('Circle ID is outside the supported range.');
    const circleId = String(circleNumber);
    const { data: identity, error: identityError } = await admin.from('chat_identities')
      .select('near_account_id').eq('auth_user_id', user.id).maybeSingle();
    if (identityError || !identity?.near_account_id) throw new Error('Sign in with your NEAR wallet before opening chat.');
    const contractId = Deno.env.get('NEAR_CONTRACT_ID') || '';
    const members = await testnetView<Member[]>(contractId, 'get_members', { circle_id: circleNumber });
    if (!contractRosterIncludesAccount(members, identity.near_account_id)) {
      throw new Error('This NEAR account is not a member of that circle.');
    }
    const { error: grantError } = await admin.from('circle_chat_memberships').upsert({
      auth_user_id: user.id,
      near_account_id: identity.near_account_id,
      circle_id: circleId,
      verified_at: new Date().toISOString(),
    }, { onConflict: 'auth_user_id,circle_id' });
    if (grantError) throw grantError;
    return jsonResponse({ circleId, accountId: identity.near_account_id, authorized: true }, 200, request);
  } catch (error) {
    return errorResponse(error, 'Could not authorize circle chat.', request);
  }
});
