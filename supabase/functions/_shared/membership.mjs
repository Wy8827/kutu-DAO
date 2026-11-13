// get_members returns the permanent financial roster. MemberStatus tracks refund
// transfers, not roster removal or chat revocation; this contract has no removal
// operation. Keep chat eligibility tied to exact account presence in that roster.
export function contractRosterIncludesAccount(members, accountId) {
  return Array.isArray(members)
    && typeof accountId === 'string'
    && members.some((member) => member && member.account_id === accountId);
}
