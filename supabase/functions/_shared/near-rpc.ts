const TESTNET_RPC = 'https://rpc.testnet.near.org';

export async function testnetView<T>(contractId: string, method: string, args: Record<string, unknown>): Promise<T> {
  if (!/^[a-z0-9._-]+\.testnet$/.test(contractId)) throw new Error('NEAR_CONTRACT_ID must be a verified Testnet account.');
  const encodedArgs = btoa(new TextEncoder().encode(JSON.stringify(args)).reduce((binary, byte) => binary + String.fromCharCode(byte), ''));
  const response = await fetch(TESTNET_RPC, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0', id: 'kutu-chat-auth', method: 'query',
      params: { request_type: 'call_function', finality: 'final', account_id: contractId, method_name: method, args_base64: encodedArgs },
    }),
  });
  if (!response.ok) throw new Error('NEAR Testnet is temporarily unavailable.');
  const body = await response.json();
  if (body.error || !Array.isArray(body.result?.result)) throw new Error('NEAR Testnet view call failed.');
  const bytes = Uint8Array.from(body.result.result as number[]);
  return JSON.parse(new TextDecoder().decode(bytes)) as T;
}

export async function hasFullAccessKey(accountId: string, publicKey: string): Promise<boolean> {
  const response = await fetch(TESTNET_RPC, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0', id: 'kutu-chat-key', method: 'query',
      params: { request_type: 'view_access_key', finality: 'final', account_id: accountId, public_key: publicKey },
    }),
  });
  if (!response.ok) return false;
  const body = await response.json();
  const permission = body.result?.permission;
  return permission === 'FullAccess' || (permission && typeof permission === 'object' && 'FullAccess' in permission);
}
