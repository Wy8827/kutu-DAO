export type PayoutRoundRecord = {
  number: number;
  payout: {
    id: number;
    recipient_id: string | null;
    amount_yocto: string;
    state: string;
  };
};

export type PayoutParentTransaction = {
  hash: string;
  label: string;
  time: number;
  circleId?: number;
  roundNumber?: number;
  payout?: boolean;
};

export function payoutHistory(rounds: PayoutRoundRecord[]): PayoutRoundRecord[] {
  return rounds
    .filter((round) => round.payout.id > 0 && ['Succeeded', 'Pending', 'Failed'].includes(round.payout.state))
    .sort((left, right) => left.number - right.number);
}

export function payoutParentTransaction(
  activity: PayoutParentTransaction[],
  circleId: number,
  roundNumber: number,
): PayoutParentTransaction | undefined {
  return activity.find((item) => item.payout && item.circleId === circleId && item.roundNumber === roundNumber);
}

export function formatYoctoNearExact(yocto: string): string {
  const amount = BigInt(yocto);
  const whole = amount / 10n ** 24n;
  const fraction = (amount % 10n ** 24n).toString().padStart(24, '0').replace(/0+$/, '');
  return `${whole}${fraction ? `.${fraction}` : ''} NEAR`;
}
