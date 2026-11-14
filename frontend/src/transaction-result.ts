export type WalletExecutionStatus = {
  SuccessValue?: unknown;
  Failure?: unknown;
};

export function getSuccessValueBase64(status: unknown): string {
  if (!status || typeof status !== 'object') {
    throw new Error('Wallet returned an invalid NEAR transaction status.');
  }

  const result = status as WalletExecutionStatus;
  if ('Failure' in result) {
    throw new Error(`NEAR transaction failed: ${JSON.stringify(result.Failure)}`);
  }
  if (typeof result.SuccessValue === 'string') return result.SuccessValue;
  throw new Error('Wallet returned an unrecognized or incomplete NEAR transaction status.');
}

export function circleIdFromSuccessValue(successValueBase64: string): number {
  if (!successValueBase64) throw new Error('The successful transaction did not return a circle ID.');

  let decoded: string;
  try {
    const binary = atob(successValueBase64);
    const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
    decoded = new TextDecoder('utf-8', { fatal: true }).decode(bytes).trim();
  } catch {
    throw new Error('The successful transaction return value is not valid Base64-encoded UTF-8.');
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(decoded);
  } catch {
    throw new Error('The successful transaction return value is not valid JSON.');
  }
  const idText = typeof parsed === 'number' && Number.isSafeInteger(parsed)
    ? String(parsed)
    : typeof parsed === 'string'
      ? parsed
      : '';

  if (!/^\d+$/.test(idText)) throw new Error('The transaction returned an invalid circle ID.');
  const id = BigInt(idText);
  if (id <= 0n || id > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error('The returned circle ID is outside the range supported by this frontend.');
  }
  return Number(id);
}

export function circlesCreatedBy<T extends { id: number; creator: string }>(
  circles: T[],
  accountId: string,
): T[] {
  if (!accountId) return [];
  const unique = new Map<string, T>();
  for (const circle of circles) {
    if (circle.creator === accountId) unique.set(String(circle.id), circle);
  }
  return [...unique.values()].sort((left, right) => left.id - right.id);
}
