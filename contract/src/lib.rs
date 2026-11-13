mod accounting;
mod model;

pub use accounting::{storage_bytes_added, storage_cost_yocto};
pub use model::{
    Circle, CircleConfig, CircleError, CircleStatus, DefaultResolution, Member, MemberStatus,
    PendingTransfer, Round, RoundStatus, TransferError, TransferKind, TransferRecipient,
    TransferState, MAX_PAYMENT_INTERVAL_NS, MIN_MEMBERS, MIN_PAYMENT_INTERVAL_NS,
};
pub use model::{MAX_DESCRIPTION_BYTES, MAX_MEMBERS, MAX_NAME_BYTES};

use near_sdk::store::LookupMap;
use near_sdk::{env, ext_contract, near, Gas, NearToken, PanicOnDefault, Promise};

#[near(serializers = [json])]
#[derive(Clone)]
pub struct CreateCircleArgs {
    pub name: String,
    pub description: Option<String>,
    pub max_members: u32,
    pub contribution_yocto: String,
    pub security_deposit_yocto: String,
    pub payment_interval_ns: u64,
}

/// Selects an immutable preallocated transfer slot. It never carries a recipient or amount.
#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferSlot {
    CircleStorageRefund,
    MemberStorageRefund(u32),
    RoundPayout(u32),
    MemberDepositRefund(u32),
    MemberCancellationRefund(u32),
}

#[near(serializers = [json])]
#[derive(Clone)]
pub struct CircleView {
    pub id: u64,
    pub creator: near_sdk::AccountId,
    pub name: String,
    pub description: Option<String>,
    pub target_members: u32,
    pub member_count: u32,
    pub contribution_yocto: String,
    pub security_deposit_yocto: String,
    pub payment_interval_ns: String,
    pub activated_at_ns: String,
    pub first_deadline_ns: String,
    pub status: String,
    pub current_round: u32,
    pub successful_payout_count: u32,
    pub paused_round: u32,
    pub paused_member_position: u32,
    pub storage_bytes_funded: u64,
    pub storage_cost_funded_yocto: String,
}

#[near(serializers = [json])]
#[derive(Clone)]
pub struct MemberView {
    pub account_id: near_sdk::AccountId,
    pub payout_position: u32,
    pub collateral_remaining_yocto: String,
    pub total_contributions_yocto: String,
    pub total_payouts_yocto: String,
    pub on_time_payments: u32,
    pub late_payments: u32,
    pub status: String,
    pub cancellation_approved: bool,
    pub cancellation_refund_yocto: String,
    pub deposit_refund: TransferView,
    pub cancellation_refund: TransferView,
    pub storage_allowance_refund: TransferView,
}

#[near(serializers = [json])]
#[derive(Clone)]
pub struct RoundMemberView {
    pub member_position: u32,
    pub direct_payment_yocto: String,
    pub collateral_substitution_yocto: String,
    pub source: String,
    pub default_resolution: String,
}

#[near(serializers = [json])]
#[derive(Clone)]
pub struct RoundView {
    pub number: u32,
    pub recipient_position: u32,
    pub opened_at_ns: String,
    pub deadline_ns: String,
    pub status: String,
    pub total_collected_yocto: String,
    pub payout: TransferView,
    pub contributions: Vec<RoundMemberView>,
}

#[near(serializers = [json])]
#[derive(Clone)]
pub struct MemberAccountingView {
    pub member: MemberView,
    pub rounds: Vec<RoundMemberView>,
}

#[near(serializers = [json])]
#[derive(Clone)]
pub struct TransferView {
    pub id: u64,
    pub kind: String,
    pub recipient_id: Option<near_sdk::AccountId>,
    pub amount_yocto: String,
    pub state: String,
    pub attempts: u32,
}

impl TransferSlot {
    fn transfer_kind(&self) -> TransferKind {
        match self {
            Self::CircleStorageRefund | Self::MemberStorageRefund(_) => {
                TransferKind::StorageAllowanceRefund
            }
            Self::RoundPayout(_) => TransferKind::Payout,
            Self::MemberDepositRefund(_) => TransferKind::DepositRefund,
            Self::MemberCancellationRefund(_) => TransferKind::CancellationRefund,
        }
    }
}

#[ext_contract(ext_transfer_callback)]
#[allow(dead_code)]
trait ExtTransferCallback {
    fn on_transfer_result(
        &mut self,
        circle_id: u64,
        slot: TransferSlot,
        transfer_id: u64,
        attempt: u32,
    ) -> bool;
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    pub schema_version: u16,
    pub next_circle_id: u64,
    pub next_transfer_id: u64,
    pub total_pending_outgoing_yocto: u128,
    pub total_storage_funded_yocto: u128,
    circles: LookupMap<u64, Circle>,
}

#[near]
impl Contract {
    #[init]
    pub fn new() -> Self {
        Self {
            schema_version: 2,
            next_circle_id: 1,
            next_transfer_id: 1,
            total_pending_outgoing_yocto: 0,
            total_storage_funded_yocto: 0,
            circles: LookupMap::new(b"c"),
        }
    }

    pub fn get_storage_price_yocto(&self) -> String {
        env::storage_byte_cost().as_yoctonear().to_string()
    }

    pub fn get_circle(&self, circle_id: u64) -> CircleView {
        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        CircleView {
            id: circle.id,
            creator: circle.creator.clone(),
            name: circle.name.clone(),
            description: circle.description.clone(),
            target_members: circle.max_members,
            member_count: circle.members.len() as u32,
            contribution_yocto: circle.contribution_yocto.to_string(),
            security_deposit_yocto: circle.security_deposit_yocto.to_string(),
            payment_interval_ns: circle.payment_interval_ns.to_string(),
            activated_at_ns: circle.activated_at_ns.to_string(),
            first_deadline_ns: circle.rounds[0].deadline_ns.to_string(),
            status: circle_status_name(&circle.status).to_owned(),
            current_round: circle.current_round,
            successful_payout_count: circle.successful_payout_count,
            paused_round: circle.paused_round,
            paused_member_position: circle.paused_member_position,
            storage_bytes_funded: circle.storage_bytes_funded,
            storage_cost_funded_yocto: circle.storage_cost_funded_yocto.to_string(),
        }
    }

    pub fn get_members(&self, circle_id: u64) -> Vec<MemberView> {
        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        circle.members.iter().map(member_view).collect()
    }

    pub fn get_round(&self, circle_id: u64, round_number: u32) -> RoundView {
        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        let round = circle
            .rounds
            .get(round_number as usize)
            .unwrap_or_else(|| env::panic_str("round not found"));
        round_view(circle, round)
    }

    pub fn get_member_accounting(
        &self,
        circle_id: u64,
        account_id: near_sdk::AccountId,
    ) -> MemberAccountingView {
        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        let member_index = circle
            .members
            .iter()
            .position(|member| member.account_id == account_id)
            .unwrap_or_else(|| env::panic_str("member not found"));
        let member = &circle.members[member_index];
        let rounds = circle
            .rounds
            .iter()
            .map(|round| round_member_view(round, member_index))
            .collect();
        MemberAccountingView {
            member: member_view(member),
            rounds,
        }
    }

    pub fn get_transfer(&self, circle_id: u64, slot: TransferSlot) -> TransferView {
        let circle = self
            .circles
            .get(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        let (transfer, recipient) = transfer_slot_view(circle, &slot);
        transfer_view(transfer, recipient)
    }

    #[payable]
    pub fn create_circle(&mut self, args: CreateCircleArgs) -> u64 {
        let contribution = parse_yocto(&args.contribution_yocto);
        let security_deposit = parse_yocto(&args.security_deposit_yocto);
        let id = self.next_circle_id;
        let next_id = id
            .checked_add(1)
            .unwrap_or_else(|| env::panic_str("circle id overflow"));
        let circle = Circle::open(
            id,
            env::predecessor_account_id(),
            CircleConfig {
                name: args.name,
                description: args.description,
                max_members: args.max_members,
                contribution_yocto: contribution,
                security_deposit_yocto: security_deposit,
                payment_interval_ns: args.payment_interval_ns,
            },
        )
        .unwrap_or_else(|error| match error {
            CircleError::ContributionTotalOverflow => {
                env::panic_str("contribution amount times membership target overflows u128")
            }
            CircleError::InvalidPaymentInterval => {
                env::panic_str("payment interval must be between 60 seconds and 365 days")
            }
            _ => env::panic_str("invalid circle configuration"),
        });
        let attached = env::attached_deposit().as_yoctonear();
        let before = env::storage_usage();
        self.circles.insert(id, circle);
        self.circles.flush();
        let after = env::storage_usage();
        let bytes = after
            .checked_sub(before)
            .unwrap_or_else(|| env::panic_str("storage usage decreased"));
        let cost = u128::from(bytes)
            .checked_mul(env::storage_byte_cost().as_yoctonear())
            .unwrap_or_else(|| env::panic_str("storage cost overflow"));
        require(attached >= cost, "insufficient storage funding");
        {
            let circle = self.circles.get_mut(&id).unwrap();
            circle.storage_bytes_funded = bytes;
            circle.storage_cost_funded_yocto = cost;
        }
        self.circles.flush();
        self.total_storage_funded_yocto = self
            .total_storage_funded_yocto
            .checked_add(cost)
            .unwrap_or_else(|| env::panic_str("storage total overflow"));
        let excess = attached - cost;
        if excess > 0 {
            self.start_storage_refund(id, TransferSlot::CircleStorageRefund, excess);
        }
        self.next_circle_id = next_id;
        emit_event("circle_created", &format!("{{\"circle_id\":{id}}}"));
        id
    }

    #[payable]
    pub fn join_circle(&mut self, circle_id: u64) -> u32 {
        let account = env::predecessor_account_id();
        let attached = env::attached_deposit().as_yoctonear();
        let circle = self
            .circles
            .get_mut(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        let collateral = circle.security_deposit_yocto;
        require(
            attached >= collateral,
            "insufficient attached deposit for collateral",
        );
        let before = env::storage_usage();
        let joined_at_ns = env::block_timestamp();
        let position = circle
            .append_member(account.clone(), collateral, 0, 0, joined_at_ns)
            .unwrap_or_else(|_| env::panic_str("join rejected"));
        self.circles.flush();
        let after = env::storage_usage();
        let bytes = after
            .checked_sub(before)
            .unwrap_or_else(|| env::panic_str("storage usage decreased"));
        let cost = u128::from(bytes)
            .checked_mul(env::storage_byte_cost().as_yoctonear())
            .unwrap_or_else(|| env::panic_str("storage cost overflow"));
        require(
            attached - collateral >= cost,
            "insufficient storage funding",
        );
        let circle = self.circles.get_mut(&circle_id).unwrap();
        let member = &mut circle.members[(position - 1) as usize];
        member.storage_bytes_added = bytes;
        member.storage_cost_paid_yocto = cost;
        circle.storage_bytes_funded = circle
            .storage_bytes_funded
            .checked_add(bytes)
            .unwrap_or_else(|| env::panic_str("storage bytes overflow"));
        circle.storage_cost_funded_yocto = circle
            .storage_cost_funded_yocto
            .checked_add(cost)
            .unwrap_or_else(|| env::panic_str("storage total overflow"));
        self.total_storage_funded_yocto = self
            .total_storage_funded_yocto
            .checked_add(cost)
            .unwrap_or_else(|| env::panic_str("storage total overflow"));
        let excess = attached - collateral - cost;
        self.circles.flush();
        if excess > 0 {
            self.start_storage_refund(
                circle_id,
                TransferSlot::MemberStorageRefund(position),
                excess,
            );
        }
        emit_event(
            "member_joined",
            &format!(
                "{{\"circle_id\":{circle_id},\"payout_position\":{position},\"account_id\":\"{account}\"}}"
            ),
        );
        let circle = self.circles.get(&circle_id).unwrap();
        if circle.status == CircleStatus::Active && circle.activated_at_ns == joined_at_ns {
            emit_event(
                "circle_activated",
                &format!(
                    "{{\"circle_id\":{circle_id},\"activated_at_ns\":\"{}\",\"first_deadline_ns\":\"{}\"}}",
                    circle.activated_at_ns, circle.rounds[0].deadline_ns
                ),
            );
        }
        position
    }

    #[payable]
    pub fn contribute(&mut self, circle_id: u64) -> bool {
        let caller = env::predecessor_account_id();
        let attached = env::attached_deposit().as_yoctonear();
        let before = env::storage_usage();
        let circle = self
            .circles
            .get_mut(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        let round_number = circle.current_round;
        let position = circle
            .members
            .iter()
            .find(|member| member.account_id == caller)
            .map(|member| member.payout_position)
            .unwrap_or_else(|| env::panic_str("caller is not a member"));
        circle
            .record_contribution(position, attached, env::block_timestamp())
            .unwrap_or_else(|_| env::panic_str("contribution rejected"));
        self.circles.flush();
        require(env::storage_usage() <= before, "contribution grew storage");
        emit_event(
            "contribution_recorded",
            &format!(
                "{{\"circle_id\":{circle_id},\"round_number\":{round_number},\"member_position\":{position},\"amount_yocto\":\"{attached}\"}}"
            ),
        );
        true
    }

    /// Applies overdue collateral substitutions in join order until funded or paused.
    pub fn process_defaults(&mut self, circle_id: u64) -> u32 {
        let before = env::storage_usage();
        let circle = self
            .circles
            .get_mut(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        let round_number = circle.current_round;
        let previous_substitutions = circle
            .rounds
            .get(round_number as usize)
            .unwrap_or_else(|| env::panic_str("round not found"))
            .collateral_substitutions_yocto
            .clone();
        let covered = circle
            .process_defaults(env::block_timestamp())
            .unwrap_or_else(|_| env::panic_str("default processing rejected"));
        self.circles.flush();
        require(
            env::storage_usage() <= before,
            "default processing grew storage",
        );
        let circle = self.circles.get(&circle_id).unwrap();
        let substitutions = &circle.rounds[round_number as usize].collateral_substitutions_yocto;
        for (member_index, amount) in substitutions.iter().enumerate() {
            if previous_substitutions[member_index] == 0 && *amount > 0 {
                let member_position = member_index + 1;
                emit_event(
                    "default_collateral_applied",
                    &format!(
                        "{{\"circle_id\":{circle_id},\"round_number\":{round_number},\"member_position\":{member_position},\"amount_yocto\":\"{amount}\"}}"
                    ),
                );
            }
        }
        if circle.status == CircleStatus::Paused {
            emit_event(
                "circle_paused",
                &format!(
                    "{{\"circle_id\":{circle_id},\"round_number\":{},\"member_position\":{}}}",
                    circle.paused_round, circle.paused_member_position
                ),
            );
        }
        covered
    }

    #[payable]
    pub fn cure_default(&mut self, circle_id: u64) -> u32 {
        let caller = env::predecessor_account_id();
        let attached = env::attached_deposit().as_yoctonear();
        let before = env::storage_usage();
        let circle = self
            .circles
            .get_mut(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        let position = circle
            .cure_default(&caller, attached)
            .unwrap_or_else(|_| env::panic_str("default cure rejected"));
        self.circles.flush();
        require(env::storage_usage() <= before, "default cure grew storage");
        emit_event(
            "default_cured",
            &format!("{{\"circle_id\":{circle_id},\"member_position\":{position}}}"),
        );
        position
    }

    /// Any account may finalize; the contract selects the recipient and exact amount.
    pub fn finalize_payout(&mut self, circle_id: u64) -> bool {
        let transfer_id = self.next_transfer_id;
        let next_transfer_id = transfer_id
            .checked_add(1)
            .unwrap_or_else(|| env::panic_str("transfer id overflow"));
        let (slot, recipient, amount) = {
            let circle = self
                .circles
                .get_mut(&circle_id)
                .unwrap_or_else(|| env::panic_str("circle not found"));
            require(
                circle.status == CircleStatus::Active,
                "circle is not active",
            );
            let round_number = circle.current_round;
            let next_round_number = round_number
                .checked_add(1)
                .unwrap_or_else(|| env::panic_str("round index overflow"));
            if next_round_number < circle.max_members {
                require(
                    env::block_timestamp()
                        .checked_add(circle.payment_interval_ns)
                        .is_some(),
                    "next round deadline would overflow",
                );
            }
            let round = circle
                .rounds
                .get_mut(round_number as usize)
                .unwrap_or_else(|| env::panic_str("round not found"));
            require(
                round.status == RoundStatus::ReadyForPayout,
                "round is not ready for payout",
            );
            let amount = circle
                .contribution_yocto
                .checked_mul(u128::from(circle.max_members))
                .unwrap_or_else(|| env::panic_str("payout amount overflow"));
            require(
                round.total_collected_yocto == amount,
                "round accounting mismatch",
            );
            let recipient_position = round.recipient_position;
            require(
                round.payout.recipient == TransferRecipient::MemberPosition(recipient_position),
                "payout recipient mismatch",
            );
            let recipient = circle
                .members
                .get((recipient_position - 1) as usize)
                .unwrap_or_else(|| env::panic_str("payout recipient missing"))
                .account_id
                .clone();
            round
                .payout
                .begin(transfer_id, amount)
                .unwrap_or_else(|_| env::panic_str("payout transfer already started"));
            round.status = RoundStatus::PayoutPending;
            (TransferSlot::RoundPayout(round_number), recipient, amount)
        };
        self.next_transfer_id = next_transfer_id;
        self.total_pending_outgoing_yocto = self
            .total_pending_outgoing_yocto
            .checked_add(amount)
            .unwrap_or_else(|| env::panic_str("pending total overflow"));
        self.circles.flush();
        emit_event(
            "payout_started",
            &format!(
                "{{\"circle_id\":{circle_id},\"round_number\":{},\"transfer_id\":{transfer_id},\"recipient_id\":\"{recipient}\",\"amount_yocto\":\"{amount}\"}}",
                match &slot {
                    TransferSlot::RoundPayout(round) => round,
                    _ => unreachable!(),
                }
            ),
        );
        Self::schedule_transfer(circle_id, slot, transfer_id, 1, recipient, amount);
        true
    }

    pub fn approve_cancellation(&mut self, circle_id: u64) -> bool {
        let caller = env::predecessor_account_id();
        let circle = self
            .circles
            .get_mut(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        require(
            circle.successful_payout_count == 0
                && circle.status != CircleStatus::Cancelled
                && circle.status != CircleStatus::Completed,
            "cancellation is unavailable",
        );
        let member = circle
            .members
            .iter_mut()
            .find(|member| member.account_id == caller)
            .unwrap_or_else(|| env::panic_str("caller is not a member"));
        require(
            !member.cancellation_approved,
            "cancellation already approved",
        );
        member.cancellation_approved = true;
        self.circles.flush();
        emit_event(
            "cancellation_approved",
            &format!("{{\"circle_id\":{circle_id},\"account_id\":\"{caller}\"}}"),
        );
        true
    }

    pub fn revoke_cancellation(&mut self, circle_id: u64) -> bool {
        let caller = env::predecessor_account_id();
        let circle = self
            .circles
            .get_mut(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        require(
            circle.successful_payout_count == 0
                && circle.status != CircleStatus::Cancelled
                && circle.status != CircleStatus::Completed,
            "cancellation is unavailable",
        );
        let member = circle
            .members
            .iter_mut()
            .find(|member| member.account_id == caller)
            .unwrap_or_else(|| env::panic_str("caller is not a member"));
        require(member.cancellation_approved, "cancellation is not approved");
        member.cancellation_approved = false;
        self.circles.flush();
        emit_event(
            "cancellation_revoked",
            &format!("{{\"circle_id\":{circle_id},\"account_id\":\"{caller}\"}}"),
        );
        true
    }

    /// Freezes every member's pre-payout refund entitlement after unanimous consent.
    pub fn finalize_cancellation(&mut self, circle_id: u64) -> bool {
        let before = env::storage_usage();
        let circle = self
            .circles
            .get_mut(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        require(
            circle.status != CircleStatus::Cancelled && circle.status != CircleStatus::Completed,
            "circle cannot be cancelled",
        );
        require(
            circle.successful_payout_count == 0,
            "a payout already succeeded",
        );
        require(!circle.has_pending_payout(), "payout transfer is pending");
        require(
            !circle.members.is_empty(),
            "empty circle must be closed by creator",
        );
        require(
            circle
                .members
                .iter()
                .all(|member| member.cancellation_approved),
            "cancellation is not unanimous",
        );
        for index in 0..circle.members.len() {
            let amount = circle
                .cancellation_refund_for(index)
                .unwrap_or_else(|_| env::panic_str("cancellation refund overflow"));
            circle.members[index].cancellation_refund_yocto = amount;
            circle.members[index].collateral_remaining_yocto = 0;
        }
        circle.status = CircleStatus::Cancelled;
        self.circles.flush();
        require(env::storage_usage() <= before, "cancellation grew storage");
        emit_event(
            "circle_cancelled",
            &format!("{{\"circle_id\":{circle_id}}}"),
        );
        true
    }

    pub fn close_empty_circle(&mut self, circle_id: u64) -> bool {
        let caller = env::predecessor_account_id();
        let circle = self
            .circles
            .get_mut(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        require(
            circle.creator == caller,
            "only creator may close empty circle",
        );
        require(circle.status == CircleStatus::Open, "circle is not open");
        require(circle.members.is_empty(), "circle has members");
        circle.status = CircleStatus::Cancelled;
        self.circles.flush();
        emit_event(
            "circle_cancelled",
            &format!("{{\"circle_id\":{circle_id}}}"),
        );
        true
    }

    pub fn claim_cancellation_refund(&mut self, circle_id: u64, position: u32) -> bool {
        let index = position
            .checked_sub(1)
            .unwrap_or_else(|| env::panic_str("member position must be one-based"))
            as usize;
        let amount = {
            let circle = self
                .circles
                .get_mut(&circle_id)
                .unwrap_or_else(|| env::panic_str("circle not found"));
            require(
                circle.status == CircleStatus::Cancelled,
                "circle is not cancelled",
            );
            let member = circle
                .members
                .get(index)
                .unwrap_or_else(|| env::panic_str("member position not found"));
            let amount = member.cancellation_refund_yocto;
            require(amount > 0, "no cancellation refund due");
            require(
                member.cancellation_refund.state == TransferState::NotStarted,
                "cancellation refund already claimed",
            );
            let member = &mut circle.members[index];
            member.cancellation_refund_yocto = 0;
            member.status = MemberStatus::RefundPending;
            amount
        };
        self.circles.flush();
        self.start_transfer(
            circle_id,
            TransferSlot::MemberCancellationRefund(position),
            amount,
        );
        emit_event(
            "refund_started",
            &format!("{{\"circle_id\":{circle_id},\"member_position\":{position},\"kind\":\"CancellationRefund\"}}"),
        );
        true
    }

    pub fn claim_deposit_refund(&mut self, circle_id: u64) -> bool {
        let caller = env::predecessor_account_id();
        let (position, amount) = {
            let circle = self
                .circles
                .get_mut(&circle_id)
                .unwrap_or_else(|| env::panic_str("circle not found"));
            require(
                circle.status == CircleStatus::Completed,
                "circle is not completed",
            );
            let member = circle
                .members
                .iter_mut()
                .find(|member| member.account_id == caller)
                .unwrap_or_else(|| env::panic_str("caller is not a member"));
            let amount = member.collateral_remaining_yocto;
            require(amount > 0, "no collateral refund due");
            require(
                member.deposit_refund.state == TransferState::NotStarted,
                "collateral refund already claimed",
            );
            member.collateral_remaining_yocto = 0;
            member.status = MemberStatus::RefundPending;
            (member.payout_position, amount)
        };
        self.start_transfer(
            circle_id,
            TransferSlot::MemberDepositRefund(position),
            amount,
        );
        emit_event(
            "refund_started",
            &format!("{{\"circle_id\":{circle_id},\"member_position\":{position},\"kind\":\"DepositRefund\"}}"),
        );
        true
    }

    /// Permissionless retry. Slot identity fixes the recipient and amount in contract state.
    pub fn retry_transfer(&mut self, circle_id: u64, slot: TransferSlot, transfer_id: u64) -> bool {
        let circle = self
            .circles
            .get_mut(&circle_id)
            .unwrap_or_else(|| env::panic_str("circle not found"));
        match &slot {
            TransferSlot::RoundPayout(round_number) => {
                require(
                    circle.status == CircleStatus::Active,
                    "payout retry is unavailable",
                );
                let next_round_number = round_number
                    .checked_add(1)
                    .unwrap_or_else(|| env::panic_str("round index overflow"));
                if next_round_number < circle.max_members {
                    require(
                        env::block_timestamp()
                            .checked_add(circle.payment_interval_ns)
                            .is_some(),
                        "next round deadline would overflow",
                    );
                }
                require(
                    circle
                        .rounds
                        .get(*round_number as usize)
                        .is_some_and(|round| round.status == RoundStatus::TransferFailed),
                    "payout is not failed",
                );
            }
            TransferSlot::MemberDepositRefund(_) => {
                require(
                    circle.status == CircleStatus::Completed,
                    "circle is not completed",
                );
            }
            TransferSlot::MemberCancellationRefund(_) => {
                require(
                    circle.status == CircleStatus::Cancelled,
                    "circle is not cancelled",
                );
            }
            _ => {}
        }
        let (transfer, recipient) = transfer_slot_mut(circle, &slot);
        require(
            transfer.kind == slot.transfer_kind(),
            "transfer kind mismatch",
        );
        let amount = transfer.amount_yocto;
        let attempt = transfer
            .retry(transfer_id)
            .unwrap_or_else(|_| env::panic_str("transfer is not retryable"));
        self.total_pending_outgoing_yocto = self
            .total_pending_outgoing_yocto
            .checked_add(amount)
            .unwrap_or_else(|| env::panic_str("pending total overflow"));
        if let TransferSlot::RoundPayout(round_number) = &slot {
            circle.rounds[*round_number as usize].status = RoundStatus::PayoutPending;
        }
        self.circles.flush();
        emit_event(
            "transfer_retried",
            &format!(
                "{{\"circle_id\":{circle_id},\"transfer_id\":{transfer_id},\"attempt\":{attempt},\"kind\":\"{}\"}}",
                transfer_kind_name(&slot.transfer_kind())
            ),
        );
        Self::schedule_transfer(circle_id, slot, transfer_id, attempt, recipient, amount);
        true
    }

    fn start_storage_refund(&mut self, circle_id: u64, slot: TransferSlot, amount: u128) {
        self.start_transfer(circle_id, slot, amount);
    }

    fn start_transfer(&mut self, circle_id: u64, slot: TransferSlot, amount: u128) {
        let transfer_id = self.next_transfer_id;
        self.next_transfer_id = transfer_id
            .checked_add(1)
            .unwrap_or_else(|| env::panic_str("transfer id overflow"));
        let circle = self.circles.get_mut(&circle_id).unwrap();
        let (transfer, recipient) = transfer_slot_mut(circle, &slot);
        require(
            transfer.kind == slot.transfer_kind(),
            "transfer kind mismatch",
        );
        let attempt = transfer
            .begin(transfer_id, amount)
            .unwrap_or_else(|_| env::panic_str("transfer slot already used"));
        self.total_pending_outgoing_yocto = self
            .total_pending_outgoing_yocto
            .checked_add(amount)
            .unwrap_or_else(|| env::panic_str("pending total overflow"));
        self.circles.flush();
        emit_event(
            "transfer_started",
            &format!(
                "{{\"circle_id\":{circle_id},\"transfer_id\":{transfer_id},\"kind\":\"{}\",\"recipient_id\":\"{recipient}\",\"amount_yocto\":\"{amount}\"}}",
                transfer_kind_name(&slot.transfer_kind())
            ),
        );
        Self::schedule_transfer(circle_id, slot, transfer_id, attempt, recipient, amount);
    }

    fn schedule_transfer(
        circle_id: u64,
        slot: TransferSlot,
        transfer_id: u64,
        attempt: u32,
        recipient: near_sdk::AccountId,
        amount: u128,
    ) {
        Promise::new(recipient)
            .transfer(NearToken::from_yoctonear(amount))
            .then(
                ext_transfer_callback::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(10))
                    .on_transfer_result(circle_id, slot, transfer_id, attempt),
            )
            .detach();
    }

    #[private]
    pub fn on_transfer_result(
        &mut self,
        circle_id: u64,
        slot: TransferSlot,
        transfer_id: u64,
        attempt: u32,
    ) -> bool {
        require(
            env::predecessor_account_id() == env::current_account_id(),
            "callback must be called by this contract",
        );
        require(
            env::promise_results_count() == 1,
            "expected one transfer result",
        );
        let succeeded = env::promise_result_checked(0, 1).is_ok();
        let amount = {
            let circle = self
                .circles
                .get_mut(&circle_id)
                .unwrap_or_else(|| env::panic_str("circle not found"));
            let (transfer, _) = transfer_slot_mut(circle, &slot);
            require(
                transfer.kind == slot.transfer_kind(),
                "transfer kind mismatch",
            );
            transfer
                .settle(transfer_id, attempt, succeeded)
                .unwrap_or_else(|_| env::panic_str("stale transfer callback"));
            transfer.amount_yocto
        };
        self.total_pending_outgoing_yocto = self
            .total_pending_outgoing_yocto
            .checked_sub(amount)
            .unwrap_or_else(|| env::panic_str("pending transfer accounting underflow"));
        let mut clock_exhausted = false;
        if let TransferSlot::RoundPayout(round_number) = &slot {
            let circle = self.circles.get_mut(&circle_id).unwrap();
            if succeeded {
                let next_round_number = round_number
                    .checked_add(1)
                    .unwrap_or_else(|| env::panic_str("round index overflow"));
                let is_final_round = next_round_number == circle.max_members;
                let callback_timestamp = env::block_timestamp();
                let next_deadline = if is_final_round {
                    None
                } else {
                    callback_timestamp.checked_add(circle.payment_interval_ns)
                };

                let round = circle
                    .rounds
                    .get_mut(*round_number as usize)
                    .unwrap_or_else(|| env::panic_str("round not found"));
                round.status = RoundStatus::Finalized;
                round.finalized_at_ns = callback_timestamp;
                circle.successful_payout_count = circle
                    .successful_payout_count
                    .checked_add(1)
                    .unwrap_or_else(|| env::panic_str("payout count overflow"));
                let recipient_position = round.recipient_position;
                let recipient = circle
                    .members
                    .get_mut((recipient_position - 1) as usize)
                    .unwrap_or_else(|| env::panic_str("payout recipient missing"));
                recipient.total_payouts_yocto = recipient
                    .total_payouts_yocto
                    .checked_add(amount)
                    .unwrap_or_else(|| env::panic_str("member payout total overflow"));
                if is_final_round {
                    circle.status = CircleStatus::Completed;
                } else {
                    circle.current_round = circle
                        .current_round
                        .checked_add(1)
                        .unwrap_or_else(|| env::panic_str("round index overflow"));
                    if let Some(deadline_ns) = next_deadline {
                        let next_round = circle
                            .rounds
                            .get_mut(next_round_number as usize)
                            .unwrap_or_else(|| env::panic_str("next round missing"));
                        next_round.opened_at_ns = callback_timestamp;
                        next_round.deadline_ns = deadline_ns;
                    } else {
                        // The transfer already succeeded. Persist that fact and freeze the
                        // circle instead of reverting the callback and making it retryable.
                        circle.status = CircleStatus::ClockExhausted;
                        clock_exhausted = true;
                    }
                }
            } else {
                let round = circle
                    .rounds
                    .get_mut(*round_number as usize)
                    .unwrap_or_else(|| env::panic_str("round not found"));
                round.status = RoundStatus::TransferFailed;
            }
        } else if let TransferSlot::MemberDepositRefund(position) = &slot {
            let circle = self.circles.get_mut(&circle_id).unwrap();
            let member = circle
                .members
                .get_mut((*position - 1) as usize)
                .unwrap_or_else(|| env::panic_str("member not found"));
            if succeeded {
                member.status = MemberStatus::Refunded;
            } else {
                member.status = MemberStatus::RefundPending;
            }
        } else if let TransferSlot::MemberCancellationRefund(position) = &slot {
            let circle = self.circles.get_mut(&circle_id).unwrap();
            let member = circle
                .members
                .get_mut((*position - 1) as usize)
                .unwrap_or_else(|| env::panic_str("member not found"));
            if succeeded {
                member.status = MemberStatus::Refunded;
            }
        }
        self.circles.flush();
        if clock_exhausted {
            emit_event(
                "circle_clock_exhausted",
                &format!(
                    "{{\"circle_id\":{circle_id},\"round_number\":{}}}",
                    match &slot {
                        TransferSlot::RoundPayout(round) => round,
                        _ => unreachable!(),
                    }
                ),
            );
        }
        emit_event(
            if succeeded {
                "transfer_succeeded"
            } else {
                "transfer_failed"
            },
            &format!(
                "{{\"circle_id\":{circle_id},\"transfer_id\":{transfer_id},\"attempt\":{attempt},\"amount_yocto\":\"{amount}\",\"kind\":\"{}\"}}",
                transfer_kind_name(&slot.transfer_kind())
            ),
        );
        if succeeded
            && matches!(slot, TransferSlot::RoundPayout(round) if round + 1 == self.circles.get(&circle_id).unwrap().max_members)
        {
            emit_event(
                "circle_completed",
                &format!("{{\"circle_id\":{circle_id}}}"),
            );
        }
        succeeded
    }
}

fn transfer_slot_mut<'a>(
    circle: &'a mut Circle,
    slot: &TransferSlot,
) -> (&'a mut PendingTransfer, near_sdk::AccountId) {
    match slot {
        TransferSlot::CircleStorageRefund => {
            require(
                circle.storage_allowance_refund.recipient == TransferRecipient::Creator,
                "transfer recipient mismatch",
            );
            (&mut circle.storage_allowance_refund, circle.creator.clone())
        }
        TransferSlot::MemberStorageRefund(position)
        | TransferSlot::MemberDepositRefund(position)
        | TransferSlot::MemberCancellationRefund(position) => {
            let index = position
                .checked_sub(1)
                .unwrap_or_else(|| env::panic_str("member position must be one-based"))
                as usize;
            let member = circle
                .members
                .get_mut(index)
                .unwrap_or_else(|| env::panic_str("member position not found"));
            let transfer = match slot {
                TransferSlot::MemberStorageRefund(_) => &mut member.storage_allowance_refund,
                TransferSlot::MemberDepositRefund(_) => &mut member.deposit_refund,
                TransferSlot::MemberCancellationRefund(_) => &mut member.cancellation_refund,
                _ => unreachable!(),
            };
            require(
                transfer.recipient == TransferRecipient::MemberPosition(*position),
                "transfer recipient mismatch",
            );
            (transfer, member.account_id.clone())
        }
        TransferSlot::RoundPayout(round_number) => {
            let round = circle
                .rounds
                .get_mut(*round_number as usize)
                .unwrap_or_else(|| env::panic_str("round not found"));
            let expected_position = round.recipient_position;
            require(
                round.payout.recipient == TransferRecipient::MemberPosition(expected_position),
                "transfer recipient mismatch",
            );
            let recipient = circle
                .members
                .get((expected_position - 1) as usize)
                .unwrap_or_else(|| env::panic_str("payout recipient missing"))
                .account_id
                .clone();
            (&mut round.payout, recipient)
        }
    }
}

fn transfer_slot_view<'a>(
    circle: &'a Circle,
    slot: &TransferSlot,
) -> (&'a PendingTransfer, Option<near_sdk::AccountId>) {
    match slot {
        TransferSlot::CircleStorageRefund => (
            &circle.storage_allowance_refund,
            Some(circle.creator.clone()),
        ),
        TransferSlot::MemberStorageRefund(position)
        | TransferSlot::MemberDepositRefund(position)
        | TransferSlot::MemberCancellationRefund(position) => {
            let index = position
                .checked_sub(1)
                .unwrap_or_else(|| env::panic_str("member position must be one-based"))
                as usize;
            let member = circle
                .members
                .get(index)
                .unwrap_or_else(|| env::panic_str("member position not found"));
            let transfer = match slot {
                TransferSlot::MemberStorageRefund(_) => &member.storage_allowance_refund,
                TransferSlot::MemberDepositRefund(_) => &member.deposit_refund,
                TransferSlot::MemberCancellationRefund(_) => &member.cancellation_refund,
                _ => unreachable!(),
            };
            (transfer, Some(member.account_id.clone()))
        }
        TransferSlot::RoundPayout(round_number) => {
            let round = circle
                .rounds
                .get(*round_number as usize)
                .unwrap_or_else(|| env::panic_str("round not found"));
            let recipient = circle
                .members
                .get((round.recipient_position - 1) as usize)
                .map(|member| member.account_id.clone());
            (&round.payout, recipient)
        }
    }
}

fn transfer_view(
    transfer: &PendingTransfer,
    recipient_id: Option<near_sdk::AccountId>,
) -> TransferView {
    TransferView {
        id: transfer.id,
        kind: transfer_kind_name(&transfer.kind).to_owned(),
        recipient_id,
        amount_yocto: transfer.amount_yocto.to_string(),
        state: transfer_state_name(&transfer.state).to_owned(),
        attempts: transfer.attempts,
    }
}

fn member_view(member: &Member) -> MemberView {
    let position = member.payout_position;
    let recipient_id = member.account_id.clone();
    MemberView {
        account_id: recipient_id.clone(),
        payout_position: position,
        collateral_remaining_yocto: member.collateral_remaining_yocto.to_string(),
        total_contributions_yocto: member.total_contributions_yocto.to_string(),
        total_payouts_yocto: member.total_payouts_yocto.to_string(),
        on_time_payments: member.on_time_payments,
        late_payments: member.late_payments,
        status: member_status_name(&member.status).to_owned(),
        cancellation_approved: member.cancellation_approved,
        cancellation_refund_yocto: member.cancellation_refund_yocto.to_string(),
        deposit_refund: transfer_view(&member.deposit_refund, Some(recipient_id.clone())),
        cancellation_refund: transfer_view(&member.cancellation_refund, Some(recipient_id.clone())),
        storage_allowance_refund: transfer_view(
            &member.storage_allowance_refund,
            Some(recipient_id),
        ),
    }
}

fn round_member_view(round: &Round, member_index: usize) -> RoundMemberView {
    let direct = round.direct_payments_yocto[member_index];
    let collateral = round.collateral_substitutions_yocto[member_index];
    let resolution = &round.default_resolution[member_index];
    let source = if direct > 0 && *resolution == DefaultResolution::Cured {
        "Cure"
    } else if direct > 0 {
        "Direct"
    } else if collateral > 0 {
        "CollateralSubstitution"
    } else {
        "Unpaid"
    };
    RoundMemberView {
        member_position: member_index as u32 + 1,
        direct_payment_yocto: direct.to_string(),
        collateral_substitution_yocto: collateral.to_string(),
        source: source.to_owned(),
        default_resolution: default_resolution_name(resolution).to_owned(),
    }
}

fn round_view(circle: &Circle, round: &Round) -> RoundView {
    let contributions = (0..circle.max_members as usize)
        .map(|index| round_member_view(round, index))
        .collect();
    let (payout, recipient) = transfer_slot_view(circle, &TransferSlot::RoundPayout(round.number));
    RoundView {
        number: round.number,
        recipient_position: round.recipient_position,
        opened_at_ns: round.opened_at_ns.to_string(),
        deadline_ns: round.deadline_ns.to_string(),
        status: round_status_name(&round.status).to_owned(),
        total_collected_yocto: round.total_collected_yocto.to_string(),
        payout: transfer_view(payout, recipient),
        contributions,
    }
}

fn circle_status_name(status: &CircleStatus) -> &'static str {
    match status {
        CircleStatus::Open => "Open",
        CircleStatus::Active => "Active",
        CircleStatus::Paused => "Paused",
        CircleStatus::Completed => "Completed",
        CircleStatus::Cancelled => "Cancelled",
        CircleStatus::ClockExhausted => "ClockExhausted",
    }
}

fn member_status_name(status: &MemberStatus) -> &'static str {
    match status {
        MemberStatus::Joined => "Joined",
        MemberStatus::RefundPending => "RefundPending",
        MemberStatus::Refunded => "Refunded",
    }
}

fn round_status_name(status: &RoundStatus) -> &'static str {
    match status {
        RoundStatus::Collecting => "Collecting",
        RoundStatus::ReadyForPayout => "ReadyForPayout",
        RoundStatus::PayoutPending => "PayoutPending",
        RoundStatus::TransferFailed => "TransferFailed",
        RoundStatus::Finalized => "Finalized",
    }
}

fn transfer_kind_name(kind: &TransferKind) -> &'static str {
    match kind {
        TransferKind::Payout => "Payout",
        TransferKind::DepositRefund => "DepositRefund",
        TransferKind::StorageAllowanceRefund => "StorageAllowanceRefund",
        TransferKind::CancellationRefund => "CancellationRefund",
    }
}

fn transfer_state_name(state: &TransferState) -> &'static str {
    match state {
        TransferState::NotStarted => "NotStarted",
        TransferState::Pending => "Pending",
        TransferState::Succeeded => "Succeeded",
        TransferState::Failed => "Failed",
    }
}

fn default_resolution_name(resolution: &DefaultResolution) -> &'static str {
    match resolution {
        DefaultResolution::NotProcessed => "NotProcessed",
        DefaultResolution::AwaitingCure => "AwaitingCure",
        DefaultResolution::Cured => "Cured",
        DefaultResolution::CollateralSubstituted => "CollateralSubstituted",
    }
}

fn emit_event(event: &str, data_object: &str) {
    env::log_str(&format!(
        "EVENT_JSON:{{\"standard\":\"kutu_dao\",\"version\":\"1.0.0\",\"event\":\"{event}\",\"data\":[{data_object}]}}"
    ));
}

fn parse_yocto(value: &str) -> u128 {
    value
        .parse()
        .unwrap_or_else(|_| env::panic_str("invalid yocto amount"))
}
fn require(condition: bool, message: &str) {
    if !condition {
        env::panic_str(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;
    use near_sdk::{AccountId, PromiseResult};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    const AFTER_FIRST_DEADLINE_NS: u64 = MIN_PAYMENT_INTERVAL_NS + 201;

    fn account(name: &str) -> AccountId {
        name.parse().expect("valid test account")
    }

    fn config(capacity: u32) -> CircleConfig {
        CircleConfig {
            name: "Test circle".to_owned(),
            description: None,
            max_members: capacity,
            contribution_yocto: 1_000,
            security_deposit_yocto: 3_000,
            payment_interval_ns: MIN_PAYMENT_INTERVAL_NS,
        }
    }

    fn create_args(capacity: u32) -> CreateCircleArgs {
        CreateCircleArgs {
            name: "Maximum-capacity test".to_owned(),
            description: Some("Storage and gas measurement case".to_owned()),
            max_members: capacity,
            contribution_yocto: "1000".to_owned(),
            security_deposit_yocto: "3000".to_owned(),
            payment_interval_ns: MIN_PAYMENT_INTERVAL_NS,
        }
    }

    fn set_caller_and_deposit(caller: &str, deposit: u128) {
        set_caller_at(caller, deposit, 1_000);
    }

    fn set_caller_at(caller: &str, deposit: u128, timestamp_ns: u64) {
        let mut context = VMContextBuilder::new();
        context.predecessor_account_id(account(caller));
        context.attached_deposit(NearToken::from_yoctonear(deposit));
        context.block_timestamp(timestamp_ns);
        testing_env!(context.build());
    }

    fn contract_with_pending_creator_refund() -> Contract {
        let mut context = VMContextBuilder::new();
        context
            .current_account_id(account("kutu.testnet"))
            .predecessor_account_id(account("creator.testnet"));
        testing_env!(context.build());
        let mut contract = Contract::new();
        let mut circle = Circle::open(1, account("creator.testnet"), config(2)).unwrap();
        circle.storage_allowance_refund.begin(1, 5_000).unwrap();
        contract.circles.insert(1, circle);
        contract.circles.flush();
        contract.next_transfer_id = 2;
        contract.total_pending_outgoing_yocto = 5_000;
        contract
    }

    fn active_contract() -> Contract {
        set_caller_at("alice.testnet", 0, 100);
        let mut contract = Contract::new();
        let mut circle = Circle::open(1, account("creator.testnet"), config(2)).unwrap();
        circle
            .append_member(account("alice.testnet"), 3_000, 0, 0, 100)
            .unwrap();
        circle
            .append_member(account("bob.testnet"), 3_000, 0, 0, 200)
            .unwrap();
        contract.circles.insert(1, circle);
        contract.circles.flush();
        contract
    }

    #[allow(deprecated)]
    fn set_callback_result_from(predecessor: &str, result: PromiseResult) {
        set_callback_result_from_at(predecessor, result, 300);
    }

    #[allow(deprecated)]
    fn set_callback_result_from_at(predecessor: &str, result: PromiseResult, timestamp_ns: u64) {
        let mut context = VMContextBuilder::new();
        context
            .current_account_id(account("kutu.testnet"))
            .predecessor_account_id(account(predecessor))
            .block_timestamp(timestamp_ns);
        near_sdk::test_utils::testing_env_with_promise_results(context.build(), result);
    }

    fn set_callback_result(result: PromiseResult) {
        set_callback_result_from("kutu.testnet", result);
    }

    #[test]
    fn max_members_and_utf8_byte_limits_are_enforced_in_contract_model() {
        let mut too_many = config(MAX_MEMBERS + 1);
        assert!(matches!(
            Circle::open(1, account("creator.testnet"), too_many.clone()),
            Err(CircleError::InvalidConfiguration)
        ));
        too_many.max_members = MAX_MEMBERS;
        too_many.name = "é".repeat(33); // 66 UTF-8 bytes
        assert!(matches!(
            Circle::open(1, account("creator.testnet"), too_many),
            Err(CircleError::InvalidConfiguration)
        ));
        let mut desc_too_long = config(2);
        desc_too_long.description = Some("界".repeat(94)); // 282 UTF-8 bytes
        assert!(matches!(
            Circle::open(1, account("creator.testnet"), desc_too_long),
            Err(CircleError::InvalidConfiguration)
        ));
        let mut boundaries = config(MAX_MEMBERS);
        boundaries.name = "é".repeat(32); // exactly 64 UTF-8 bytes
        boundaries.description = Some("界".repeat(93) + "a"); // exactly 280 bytes
        assert!(Circle::open(1, account("creator.testnet"), boundaries).is_ok());
    }

    #[test]
    fn creation_validates_full_round_amount_against_configured_target() {
        let creator = account("creator.testnet");

        let mut normal = config(2);
        normal.contribution_yocto = 1_000;
        assert!(Circle::open(1, creator.clone(), normal).is_ok());

        let mut maximum_valid = config(2);
        maximum_valid.contribution_yocto = u128::MAX / 2;
        let circle = Circle::open(1, creator.clone(), maximum_valid).unwrap();
        assert_eq!(
            circle.contribution_yocto * u128::from(circle.max_members),
            u128::MAX - 1
        );

        let mut first_overflow = config(2);
        first_overflow.contribution_yocto = (u128::MAX / 2) + 1;
        assert!(matches!(
            Circle::open(1, creator.clone(), first_overflow),
            Err(CircleError::ContributionTotalOverflow)
        ));

        // The same amount is safe for target 2 but must be rejected for target 3.
        let amount = u128::MAX / 2;
        let mut target_two = config(2);
        target_two.contribution_yocto = amount;
        assert!(Circle::open(1, creator.clone(), target_two).is_ok());
        let mut target_three = config(3);
        target_three.contribution_yocto = amount;
        assert!(matches!(
            Circle::open(1, creator.clone(), target_three),
            Err(CircleError::ContributionTotalOverflow)
        ));

        let mut zero = config(2);
        zero.contribution_yocto = 0;
        assert!(matches!(
            Circle::open(1, creator.clone(), zero),
            Err(CircleError::InvalidConfiguration)
        ));
        let mut invalid_target = config(1);
        invalid_target.contribution_yocto = u128::MAX;
        assert!(matches!(
            Circle::open(1, creator, invalid_target),
            Err(CircleError::InvalidConfiguration)
        ));
    }

    #[test]
    fn payment_interval_enforces_approved_minimum_and_maximum() {
        let creator = account("creator.testnet");
        let mut minimum = config(2);
        minimum.payment_interval_ns = MIN_PAYMENT_INTERVAL_NS;
        assert!(Circle::open(1, creator.clone(), minimum).is_ok());

        let mut maximum = config(2);
        maximum.payment_interval_ns = MAX_PAYMENT_INTERVAL_NS;
        assert!(Circle::open(1, creator.clone(), maximum).is_ok());

        let mut below_minimum = config(2);
        below_minimum.payment_interval_ns = MIN_PAYMENT_INTERVAL_NS - 1;
        assert!(matches!(
            Circle::open(1, creator.clone(), below_minimum),
            Err(CircleError::InvalidPaymentInterval)
        ));

        let mut above_maximum = config(2);
        above_maximum.payment_interval_ns = MAX_PAYMENT_INTERVAL_NS + 1;
        assert!(matches!(
            Circle::open(1, creator, above_maximum),
            Err(CircleError::InvalidPaymentInterval)
        ));
    }

    #[test]
    fn activation_timestamp_overflow_rejects_final_join_without_mutation() {
        let mut circle = Circle::open(1, account("creator.testnet"), config(2)).unwrap();
        circle
            .append_member(account("alice.testnet"), 3_000, 0, 0, 100)
            .unwrap();
        let overflow_timestamp = u64::MAX - MIN_PAYMENT_INTERVAL_NS + 1;
        assert_eq!(
            circle.append_member(account("bob.testnet"), 3_000, 0, 0, overflow_timestamp),
            Err(CircleError::DeadlineOverflow)
        );
        assert_eq!(circle.members.len(), 1);
        assert_eq!(circle.status, CircleStatus::Open);
    }

    #[test]
    fn payout_deadline_overflow_is_preflighted_and_callback_never_reopens_successful_payout() {
        let mut preflighted = active_contract();
        contribute_both_members(&mut preflighted, 230);
        set_caller_at("anyone.testnet", 0, u64::MAX);
        let rejected = catch_unwind(AssertUnwindSafe(|| preflighted.finalize_payout(1)));
        assert!(rejected.is_err());
        let circle = preflighted.circles.get(&1).unwrap();
        assert_eq!(circle.rounds[0].status, RoundStatus::ReadyForPayout);
        assert_eq!(circle.rounds[0].payout.state, TransferState::NotStarted);
        assert_eq!(preflighted.total_pending_outgoing_yocto, 0);

        let mut retry_preflight = active_contract();
        contribute_both_members(&mut retry_preflight, 230);
        set_caller_at("anyone.testnet", 0, 300);
        assert!(retry_preflight.finalize_payout(1));
        set_callback_result(PromiseResult::Failed);
        assert!(!retry_preflight.on_transfer_result(1, TransferSlot::RoundPayout(0), 1, 1));
        set_caller_at("anyone.testnet", 0, u64::MAX);
        let retry_rejected = catch_unwind(AssertUnwindSafe(|| {
            retry_preflight.retry_transfer(1, TransferSlot::RoundPayout(0), 1)
        }));
        assert!(retry_rejected.is_err());
        let circle = retry_preflight.circles.get(&1).unwrap();
        assert_eq!(circle.rounds[0].status, RoundStatus::TransferFailed);
        assert_eq!(circle.rounds[0].payout.state, TransferState::Failed);
        assert_eq!(circle.rounds[0].payout.attempts, 1);
        assert_eq!(retry_preflight.total_pending_outgoing_yocto, 0);

        let mut callback_edge = active_contract();
        contribute_both_members(&mut callback_edge, 230);
        set_caller_at("anyone.testnet", 0, 300);
        assert!(callback_edge.finalize_payout(1));
        set_callback_result_from_at("kutu.testnet", PromiseResult::Successful(vec![]), u64::MAX);
        assert!(callback_edge.on_transfer_result(1, TransferSlot::RoundPayout(0), 1, 1));

        let circle = callback_edge.circles.get(&1).unwrap();
        assert_eq!(circle.status, CircleStatus::ClockExhausted);
        assert_eq!(circle.rounds[0].status, RoundStatus::Finalized);
        assert_eq!(circle.rounds[0].payout.state, TransferState::Succeeded);
        assert_eq!(circle.successful_payout_count, 1);
        assert_eq!(circle.current_round, 1);
        assert_eq!(callback_edge.total_pending_outgoing_yocto, 0);
    }

    #[test]
    fn create_and_join_measure_max_capacity_storage_and_assign_all_positions() {
        let mut contract = Contract::new();
        set_caller_and_deposit("creator.testnet", 100 * 10u128.pow(24));
        let create_gas_before = env::used_gas().as_gas();
        let id = contract.create_circle(create_args(MAX_MEMBERS));
        let create_gas = env::used_gas().as_gas().saturating_sub(create_gas_before);
        let created = contract.circles.get(&id).unwrap();
        let create_bytes = created.storage_bytes_funded;
        let create_cost = created.storage_cost_funded_yocto;
        assert!(create_bytes > 0);
        assert_eq!(
            create_cost,
            u128::from(create_bytes) * env::storage_byte_cost().as_yoctonear()
        );

        let mut join_growth = 0u64;
        let mut join_gas_total = 0u64;
        let mut max_join_gas = 0u64;
        for i in 0..MAX_MEMBERS {
            let caller = format!("member{i}.testnet");
            set_caller_and_deposit(&caller, 100 * 10u128.pow(24));
            let before = env::storage_usage();
            let gas_before = env::used_gas().as_gas();
            let position = contract.join_circle(id);
            let join_gas = env::used_gas().as_gas().saturating_sub(gas_before);
            join_gas_total += join_gas;
            max_join_gas = max_join_gas.max(join_gas);
            let after = env::storage_usage();
            join_growth += after - before;
            assert_eq!(position, i + 1);
        }
        let circle = contract.circles.get(&id).unwrap();
        assert_eq!(circle.members.len(), MAX_MEMBERS as usize);
        assert_eq!(circle.status, CircleStatus::Active);
        assert_eq!(
            circle
                .members
                .iter()
                .map(|m| m.payout_position)
                .collect::<Vec<_>>(),
            (1..=MAX_MEMBERS).collect::<Vec<_>>()
        );
        assert_eq!(circle.rounds.len(), MAX_MEMBERS as usize);
        for round in &circle.rounds {
            assert_eq!(round.direct_payments_yocto.len(), MAX_MEMBERS as usize);
            assert_eq!(
                round.collateral_substitutions_yocto.len(),
                MAX_MEMBERS as usize
            );
        }
        let refund_ids = std::iter::once(circle.storage_allowance_refund.id)
            .chain(
                circle
                    .members
                    .iter()
                    .map(|member| member.storage_allowance_refund.id),
            )
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(refund_ids.len(), MAX_MEMBERS as usize + 1);
        assert_eq!(
            refund_ids.iter().copied().collect::<Vec<_>>(),
            (1..=MAX_MEMBERS as u64 + 1).collect::<Vec<_>>()
        );
        assert!(join_growth > 0);
        println!("MAX_CAPACITY_METRICS create_bytes={create_bytes} create_cost_yocto={create_cost} create_mock_gas={create_gas} join_bytes={join_growth} join_cost_yocto={} joins_mock_gas_total={join_gas_total} max_join_mock_gas={max_join_gas}", u128::from(join_growth) * env::storage_byte_cost().as_yoctonear());
    }

    #[test]
    fn create_and_join_keep_collateral_storage_cost_and_excess_refund_separate() {
        let mut contract = Contract::new();
        let create_attached = 10u128.pow(24);
        set_caller_and_deposit("creator.testnet", create_attached);
        let id = contract.create_circle(create_args(2));

        let circle = contract.circles.get(&id).unwrap();
        let create_refund = &circle.storage_allowance_refund;
        assert_eq!(circle.members.len(), 0);
        assert!(circle.storage_cost_funded_yocto < create_attached);
        assert_eq!(create_refund.recipient, TransferRecipient::Creator);
        assert_eq!(create_refund.state, TransferState::Pending);
        assert_eq!(
            create_refund.amount_yocto,
            create_attached - circle.storage_cost_funded_yocto
        );
        let create_refund_amount = create_refund.amount_yocto;
        let create_cost = circle.storage_cost_funded_yocto;

        let join_excess = 2 * 10u128.pow(24);
        let attached = 3_000 + join_excess;
        set_caller_and_deposit("creator.testnet", attached);
        let position = contract.join_circle(id);
        assert_eq!(position, 1);

        let circle = contract.circles.get(&id).unwrap();
        let member = &circle.members[0];
        assert_eq!(member.collateral_remaining_yocto, 3_000);
        assert_eq!(
            member.storage_cost_paid_yocto,
            u128::from(member.storage_bytes_added) * env::storage_byte_cost().as_yoctonear()
        );
        assert!(member.storage_cost_paid_yocto <= join_excess);
        assert_eq!(
            member.storage_allowance_refund.recipient,
            TransferRecipient::MemberPosition(1)
        );
        assert_eq!(
            member.storage_allowance_refund.amount_yocto,
            join_excess - member.storage_cost_paid_yocto
        );
        assert_eq!(
            member.storage_allowance_refund.state,
            TransferState::Pending
        );
        assert_eq!(
            contract.total_pending_outgoing_yocto,
            create_refund_amount + member.storage_allowance_refund.amount_yocto
        );
        assert_eq!(
            contract.total_storage_funded_yocto,
            create_cost + member.storage_cost_paid_yocto
        );
    }

    #[test]
    #[should_panic(expected = "insufficient storage funding")]
    fn create_rejects_insufficient_storage_payment() {
        let mut contract = Contract::new();
        set_caller_and_deposit("creator.testnet", 1);
        contract.create_circle(create_args(2));
    }

    #[test]
    #[should_panic(expected = "insufficient storage funding")]
    fn join_rejects_collateral_without_separate_storage_payment() {
        let mut contract = Contract::new();
        set_caller_and_deposit("creator.testnet", 100 * 10u128.pow(24));
        let id = contract.create_circle(create_args(2));
        set_caller_and_deposit("member.testnet", 3_000);
        contract.join_circle(id);
    }

    #[test]
    fn creator_is_not_auto_joined_and_join_order_assigns_positions() {
        let mut circle = Circle::open(1, account("creator.testnet"), config(3)).unwrap();
        assert!(circle.members.is_empty());
        assert_eq!(circle.status, CircleStatus::Open);

        let first = circle
            .append_member(account("alice.testnet"), 3_000, 4, 40, 100)
            .unwrap();
        let second = circle
            .append_member(account("bob.testnet"), 3_000, 4, 40, 110)
            .unwrap();
        assert_eq!((first, second), (1, 2));
        assert_eq!(circle.rounds[0].recipient_position, 1);
        assert_eq!(circle.rounds[1].recipient_position, 2);
        assert_eq!(circle.status, CircleStatus::Open);

        let third = circle
            .append_member(account("creator.testnet"), 3_000, 6, 60, 120)
            .unwrap();
        assert_eq!(third, 3);
        assert_eq!(circle.status, CircleStatus::Active);
        assert_eq!(circle.members[2].account_id, account("creator.testnet"));
        assert_eq!(circle.members[2].payout_position, 3);
        assert_eq!(circle.activated_at_ns, 120);
        assert_eq!(circle.rounds[0].opened_at_ns, 120);
        assert_eq!(circle.rounds[0].deadline_ns, 120 + MIN_PAYMENT_INTERVAL_NS);
    }

    #[test]
    fn circle_rejects_duplicate_full_and_bad_collateral_joins() {
        let mut circle = Circle::open(1, account("creator.testnet"), config(2)).unwrap();
        assert_eq!(
            circle.append_member(account("alice.testnet"), 2_999, 1, 10, 100),
            Err(CircleError::IncorrectCollateral)
        );
        circle
            .append_member(account("alice.testnet"), 3_000, 1, 10, 100)
            .unwrap();
        assert_eq!(
            circle.append_member(account("alice.testnet"), 3_000, 0, 0, 110),
            Err(CircleError::DuplicateMember)
        );
        circle
            .append_member(account("bob.testnet"), 3_000, 2, 20, 120)
            .unwrap();
        assert_eq!(circle.status, CircleStatus::Active);
        assert_eq!(
            circle.append_member(account("carol.testnet"), 3_000, 0, 0, 130),
            Err(CircleError::NotOpen)
        );
        assert_eq!(circle.members.len(), 2);
    }

    #[test]
    fn round_slots_are_preallocated_to_member_capacity() {
        let circle = Circle::open(1, account("creator.testnet"), config(4)).unwrap();
        assert_eq!(circle.rounds.len(), 4);
        for round in &circle.rounds {
            assert_eq!(round.direct_payments_yocto.len(), 4);
            assert_eq!(round.collateral_substitutions_yocto.len(), 4);
        }
    }

    #[test]
    fn retry_preserves_transfer_id_recipient_and_amount() {
        let recipient = TransferRecipient::MemberPosition(2);
        let mut transfer = PendingTransfer::new_slot(TransferKind::Payout, recipient.clone());
        transfer.begin(7, 9_000).unwrap();
        transfer.settle(7, 1, false).unwrap();
        transfer.retry(7).unwrap();
        assert_eq!(
            transfer.settle(7, 1, true),
            Err(TransferError::StaleCallback)
        );
        assert_eq!(transfer.id, 7);
        assert_eq!(transfer.recipient, recipient);
        assert_eq!(transfer.amount_yocto, 9_000);
        assert_eq!(transfer.attempts, 2);
        assert_eq!(transfer.state, TransferState::Pending);
    }

    #[test]
    fn stale_callback_cannot_settle_a_new_or_already_settled_transfer() {
        let mut transfer = PendingTransfer::new_slot(
            TransferKind::DepositRefund,
            TransferRecipient::MemberPosition(1),
        );
        transfer.begin(10, 500).unwrap();
        assert_eq!(
            transfer.settle(9, 1, true),
            Err(TransferError::StaleCallback)
        );
        transfer.settle(10, 1, true).unwrap();
        assert_eq!(
            transfer.settle(10, 1, true),
            Err(TransferError::StaleCallback)
        );
    }

    #[test]
    fn storage_refund_success_and_duplicate_callback_are_safe() {
        let mut contract = contract_with_pending_creator_refund();
        set_callback_result(PromiseResult::Successful(vec![]));
        assert!(contract.on_transfer_result(1, TransferSlot::CircleStorageRefund, 1, 1));
        let transfer = &contract.circles.get(&1).unwrap().storage_allowance_refund;
        assert_eq!(transfer.state, TransferState::Succeeded);
        assert_eq!(transfer.recipient, TransferRecipient::Creator);
        assert_eq!(transfer.amount_yocto, 5_000);
        assert_eq!(contract.total_pending_outgoing_yocto, 0);

        let duplicate = catch_unwind(AssertUnwindSafe(|| {
            contract.on_transfer_result(1, TransferSlot::CircleStorageRefund, 1, 1)
        }));
        assert!(duplicate.is_err());
        assert_eq!(
            contract
                .circles
                .get(&1)
                .unwrap()
                .storage_allowance_refund
                .state,
            TransferState::Succeeded
        );
    }

    #[test]
    fn external_account_cannot_invoke_transfer_callback() {
        let mut contract = contract_with_pending_creator_refund();
        set_callback_result_from("attacker.testnet", PromiseResult::Successful(vec![]));
        let forged = catch_unwind(AssertUnwindSafe(|| {
            contract.on_transfer_result(1, TransferSlot::CircleStorageRefund, 1, 1)
        }));
        assert!(forged.is_err());
        assert_eq!(
            contract
                .circles
                .get(&1)
                .unwrap()
                .storage_allowance_refund
                .state,
            TransferState::Pending
        );
        assert_eq!(contract.total_pending_outgoing_yocto, 5_000);
    }

    #[test]
    fn failed_storage_refund_can_be_retried_without_changing_entitlement() {
        let mut contract = contract_with_pending_creator_refund();
        set_callback_result(PromiseResult::Failed);
        assert!(!contract.on_transfer_result(1, TransferSlot::CircleStorageRefund, 1, 1));
        let transfer = &contract.circles.get(&1).unwrap().storage_allowance_refund;
        assert_eq!(transfer.state, TransferState::Failed);
        assert_eq!(transfer.id, 1);
        assert_eq!(transfer.recipient, TransferRecipient::Creator);
        assert_eq!(transfer.amount_yocto, 5_000);
        assert_eq!(contract.total_pending_outgoing_yocto, 0);

        set_caller_and_deposit("random.testnet", 0);
        assert!(contract.retry_transfer(1, TransferSlot::CircleStorageRefund, 1));
        let retried = &contract.circles.get(&1).unwrap().storage_allowance_refund;
        assert_eq!(retried.state, TransferState::Pending);
        assert_eq!(retried.attempts, 2);
        assert_eq!(retried.id, 1);
        assert_eq!(retried.recipient, TransferRecipient::Creator);
        assert_eq!(retried.amount_yocto, 5_000);
        assert_eq!(contract.total_pending_outgoing_yocto, 5_000);

        let duplicate_retry = catch_unwind(AssertUnwindSafe(|| {
            contract.retry_transfer(1, TransferSlot::CircleStorageRefund, 1)
        }));
        assert!(duplicate_retry.is_err());
        assert_eq!(contract.total_pending_outgoing_yocto, 5_000);

        set_callback_result(PromiseResult::Successful(vec![]));
        let stale_attempt = catch_unwind(AssertUnwindSafe(|| {
            contract.on_transfer_result(1, TransferSlot::CircleStorageRefund, 1, 1)
        }));
        assert!(stale_attempt.is_err());
        assert_eq!(
            contract
                .circles
                .get(&1)
                .unwrap()
                .storage_allowance_refund
                .state,
            TransferState::Pending
        );

        assert!(contract.on_transfer_result(1, TransferSlot::CircleStorageRefund, 1, 2));
        assert_eq!(
            contract
                .circles
                .get(&1)
                .unwrap()
                .storage_allowance_refund
                .state,
            TransferState::Succeeded
        );
        assert_eq!(contract.total_pending_outgoing_yocto, 0);
    }

    #[test]
    fn lifecycle_field_updates_do_not_grow_preallocated_circle_storage() {
        testing_env!(VMContextBuilder::new().build());
        let mut circle = Circle::open(1, account("creator.testnet"), config(MIN_MEMBERS)).unwrap();
        circle
            .append_member(account("alice.testnet"), 3_000, 8, 80, 100)
            .unwrap();
        let mut circles = LookupMap::new(&b"test-circles"[..]);
        circles.insert(1, circle);
        circles.flush();
        let before = env::storage_usage();

        {
            let circle = circles.get_mut(&1).unwrap();
            circle.rounds[0].direct_payments_yocto[0] = 1_000;
            circle.rounds[0].total_collected_yocto = 1_000;
            circle.rounds[0].status = RoundStatus::PayoutPending;
            circle.rounds[0].payout.begin(1, 1_000).unwrap();
            circle.rounds[0].finalized_at_ns = 200;
            circle.members[0].total_contributions_yocto = 1_000;
            circle.members[0].status = MemberStatus::RefundPending;
            circle.members[0].deposit_refund.begin(2, 3_000).unwrap();
            circle.members[0]
                .deposit_refund
                .settle(2, 1, false)
                .unwrap();
            circle.members[0].deposit_refund.retry(2).unwrap();
            circle.storage_allowance_refund.begin(3, 10).unwrap();
            circle.status = CircleStatus::Active;
        }
        circles.flush();
        assert_eq!(env::storage_usage(), before);
    }

    #[test]
    fn exact_contribution_is_member_only_once_and_deadline_bounded() {
        let mut contract = active_contract();
        set_caller_at("alice.testnet", 1_000, 230);
        assert!(contract.contribute(1));
        let circle = contract.circles.get(&1).unwrap();
        assert_eq!(circle.rounds[0].direct_payments_yocto[0], 1_000);
        assert_eq!(circle.members[0].total_contributions_yocto, 1_000);

        let duplicate = catch_unwind(AssertUnwindSafe(|| contract.contribute(1)));
        assert!(duplicate.is_err());
        set_caller_at("mallory.testnet", 1_000, 230);
        let unauthorized = catch_unwind(AssertUnwindSafe(|| contract.contribute(1)));
        assert!(unauthorized.is_err());
        set_caller_at("bob.testnet", 999, 230);
        let wrong_amount = catch_unwind(AssertUnwindSafe(|| contract.contribute(1)));
        assert!(wrong_amount.is_err());
        set_caller_at("bob.testnet", 1_000, AFTER_FIRST_DEADLINE_NS);
        let after_deadline = catch_unwind(AssertUnwindSafe(|| contract.contribute(1)));
        assert!(after_deadline.is_err());

        let circle = contract.circles.get(&1).unwrap();
        assert_eq!(circle.rounds[0].direct_payments_yocto, vec![1_000, 0]);
        assert_eq!(circle.rounds[0].total_collected_yocto, 1_000);
        assert_eq!(circle.members[1].total_contributions_yocto, 0);
    }

    #[test]
    fn overdue_defaults_substitute_full_collateral_once_or_pause_without_deduction() {
        let mut covered = active_contract();
        set_caller_at("anyone.testnet", 0, AFTER_FIRST_DEADLINE_NS);
        assert_eq!(covered.process_defaults(1), 2);
        let circle = covered.circles.get(&1).unwrap();
        assert_eq!(circle.status, CircleStatus::Active);
        assert_eq!(circle.rounds[0].total_collected_yocto, 2_000);
        assert_eq!(
            circle.rounds[0].collateral_substitutions_yocto,
            vec![1_000, 1_000]
        );
        assert_eq!(circle.members[0].collateral_remaining_yocto, 2_000);
        assert_eq!(circle.members[1].collateral_remaining_yocto, 2_000);
        assert_eq!(circle.rounds[0].status, RoundStatus::ReadyForPayout);
        let duplicate_processing = catch_unwind(AssertUnwindSafe(|| covered.process_defaults(1)));
        assert!(duplicate_processing.is_err());
        let circle = covered.circles.get(&1).unwrap();
        assert_eq!(circle.rounds[0].total_collected_yocto, 2_000);

        let mut paused = active_contract();
        paused.circles.get_mut(&1).unwrap().members[0].collateral_remaining_yocto = 999;
        paused.circles.flush();
        set_caller_at("anyone.testnet", 0, AFTER_FIRST_DEADLINE_NS);
        assert_eq!(paused.process_defaults(1), 0);
        let circle = paused.circles.get(&1).unwrap();
        assert_eq!(circle.status, CircleStatus::Paused);
        assert_eq!(circle.paused_member_position, 1);
        assert_eq!(circle.members[0].collateral_remaining_yocto, 999);
        assert_eq!(circle.rounds[0].collateral_substitutions_yocto[0], 0);

        set_caller_at("bob.testnet", 1_000, 232);
        let wrong_curer = catch_unwind(AssertUnwindSafe(|| paused.cure_default(1)));
        assert!(wrong_curer.is_err());
        set_caller_at("alice.testnet", 999, 232);
        let underpayment = catch_unwind(AssertUnwindSafe(|| paused.cure_default(1)));
        assert!(underpayment.is_err());
        let circle = paused.circles.get(&1).unwrap();
        assert_eq!(circle.status, CircleStatus::Paused);
        assert_eq!(circle.members[0].collateral_remaining_yocto, 999);
        assert_eq!(circle.rounds[0].direct_payments_yocto[0], 0);

        set_caller_at("alice.testnet", 1_000, 232);
        assert_eq!(paused.cure_default(1), 1);
        let circle = paused.circles.get(&1).unwrap();
        assert_eq!(circle.status, CircleStatus::Active);
        assert_eq!(circle.members[0].collateral_remaining_yocto, 999);
        assert_eq!(circle.rounds[0].direct_payments_yocto[0], 1_000);
        assert_eq!(
            circle.rounds[0].default_resolution[0],
            DefaultResolution::Cured
        );
        set_caller_at("anyone.testnet", 0, AFTER_FIRST_DEADLINE_NS + 2);
        assert_eq!(paused.process_defaults(1), 1);
        let circle = paused.circles.get(&1).unwrap();
        assert_eq!(circle.rounds[0].status, RoundStatus::ReadyForPayout);
        assert_eq!(circle.rounds[0].total_collected_yocto, 2_000);
        assert_eq!(circle.members[0].collateral_remaining_yocto, 999);
        assert_eq!(circle.members[1].collateral_remaining_yocto, 2_000);
    }

    fn contribute_both_members(contract: &mut Contract, timestamp_ns: u64) {
        set_caller_at("alice.testnet", 1_000, timestamp_ns);
        contract.contribute(1);
        set_caller_at("bob.testnet", 1_000, timestamp_ns);
        contract.contribute(1);
    }

    fn assert_circle_financial_invariants(contract: &Contract, circle_id: u64) {
        let circle = contract.circles.get(&circle_id).unwrap();
        let expected_payout = circle.contribution_yocto * u128::from(circle.max_members);
        let mut contributions_by_member = vec![0u128; circle.members.len()];
        let mut payouts_by_member = vec![0u128; circle.members.len()];
        let mut successful_payouts = 0u32;
        let mut pending_amount = 0u128;

        if circle.storage_allowance_refund.state == TransferState::Pending {
            pending_amount += circle.storage_allowance_refund.amount_yocto;
        }
        for (round_index, round) in circle.rounds.iter().enumerate() {
            assert_eq!(round.direct_payments_yocto.len(), circle.members.len());
            assert_eq!(
                round.collateral_substitutions_yocto.len(),
                circle.members.len()
            );
            let mut round_total = 0u128;
            for member_index in 0..circle.members.len() {
                let direct = round.direct_payments_yocto[member_index];
                let substituted = round.collateral_substitutions_yocto[member_index];
                assert!(direct == 0 || substituted == 0);
                round_total += direct + substituted;
                contributions_by_member[member_index] += direct + substituted;
                match round.default_resolution[member_index] {
                    DefaultResolution::Cured => {
                        assert_eq!(direct, circle.contribution_yocto);
                        assert_eq!(substituted, 0);
                    }
                    DefaultResolution::CollateralSubstituted => {
                        assert_eq!(direct, 0);
                        assert_eq!(substituted, circle.contribution_yocto);
                    }
                    _ => {}
                }
            }
            assert_eq!(round.total_collected_yocto, round_total);
            if round.payout.state == TransferState::Pending {
                pending_amount += round.payout.amount_yocto;
                assert_eq!(round.status, RoundStatus::PayoutPending);
            }
            if round.payout.state == TransferState::Succeeded {
                successful_payouts += 1;
                assert_eq!(round.status, RoundStatus::Finalized);
                assert_eq!(round.payout.amount_yocto, expected_payout);
                payouts_by_member[(round.recipient_position - 1) as usize] += expected_payout;
            }
            if round_index < circle.current_round as usize {
                assert_eq!(round.payout.state, TransferState::Succeeded);
            }
        }

        for (index, member) in circle.members.iter().enumerate() {
            assert_eq!(
                member.total_contributions_yocto,
                contributions_by_member[index]
            );
            assert_eq!(member.total_payouts_yocto, payouts_by_member[index]);
            if member.deposit_refund.state == TransferState::Pending {
                pending_amount += member.deposit_refund.amount_yocto;
            }
            if member.cancellation_refund.state == TransferState::Pending {
                pending_amount += member.cancellation_refund.amount_yocto;
            }
            if member.storage_allowance_refund.state == TransferState::Pending {
                pending_amount += member.storage_allowance_refund.amount_yocto;
            }
            if circle.status != CircleStatus::Cancelled {
                let substituted = circle
                    .rounds
                    .iter()
                    .map(|round| round.collateral_substitutions_yocto[index])
                    .sum::<u128>();
                assert_eq!(
                    member.collateral_remaining_yocto
                        + substituted
                        + member.deposit_refund.amount_yocto,
                    circle.security_deposit_yocto
                );
            }
        }
        assert_eq!(circle.successful_payout_count, successful_payouts);
        assert_eq!(
            circle
                .members
                .iter()
                .map(|m| m.total_contributions_yocto)
                .sum::<u128>(),
            circle
                .rounds
                .iter()
                .map(|r| r.total_collected_yocto)
                .sum::<u128>()
        );
        assert_eq!(contract.total_pending_outgoing_yocto, pending_amount);
    }

    #[test]
    fn financial_invariants_hold_through_contribution_collateral_failure_and_retry() {
        let mut contract = active_contract();
        assert_circle_financial_invariants(&contract, 1);

        set_caller_at("alice.testnet", 1_000, 230);
        contract.contribute(1);
        assert_circle_financial_invariants(&contract, 1);

        set_caller_at("anyone.testnet", 0, AFTER_FIRST_DEADLINE_NS);
        assert_eq!(contract.process_defaults(1), 1);
        assert_circle_financial_invariants(&contract, 1);

        set_caller_at("anyone.testnet", 0, AFTER_FIRST_DEADLINE_NS + 1);
        assert!(contract.finalize_payout(1));
        assert_circle_financial_invariants(&contract, 1);

        set_callback_result(PromiseResult::Failed);
        assert!(!contract.on_transfer_result(1, TransferSlot::RoundPayout(0), 1, 1));
        assert_circle_financial_invariants(&contract, 1);

        set_caller_at("random.testnet", 0, AFTER_FIRST_DEADLINE_NS + 2);
        assert!(contract.retry_transfer(1, TransferSlot::RoundPayout(0), 1));
        assert_circle_financial_invariants(&contract, 1);

        set_callback_result(PromiseResult::Successful(vec![]));
        assert!(contract.on_transfer_result(1, TransferSlot::RoundPayout(0), 1, 2));
        assert_circle_financial_invariants(&contract, 1);
    }

    #[test]
    fn cancellation_requires_current_unanimity_and_cannot_follow_a_payout() {
        let mut contract = active_contract();
        set_caller_at("alice.testnet", 0, 210);
        assert!(contract.approve_cancellation(1));
        let non_unanimous = catch_unwind(AssertUnwindSafe(|| contract.finalize_cancellation(1)));
        assert!(non_unanimous.is_err());

        set_caller_at("bob.testnet", 0, 211);
        let revoke_unapproved = catch_unwind(AssertUnwindSafe(|| contract.revoke_cancellation(1)));
        assert!(revoke_unapproved.is_err());
        assert!(contract.approve_cancellation(1));
        set_caller_at("alice.testnet", 0, 212);
        assert!(contract.revoke_cancellation(1));
        let revoked_consent = catch_unwind(AssertUnwindSafe(|| contract.finalize_cancellation(1)));
        assert!(revoked_consent.is_err());
        assert!(contract.approve_cancellation(1));
        set_caller_at("bob.testnet", 0, 213);
        let duplicate_approval =
            catch_unwind(AssertUnwindSafe(|| contract.approve_cancellation(1)));
        assert!(duplicate_approval.is_err());
        set_caller_at("anyone.testnet", 0, 214);
        assert!(contract.finalize_cancellation(1));
        let duplicate_cancellation =
            catch_unwind(AssertUnwindSafe(|| contract.finalize_cancellation(1)));
        assert!(duplicate_cancellation.is_err());

        let mut paid = active_contract();
        contribute_both_members(&mut paid, 230);
        set_caller_at("anyone.testnet", 0, 231);
        assert!(paid.finalize_payout(1));
        set_callback_result(PromiseResult::Successful(vec![]));
        assert!(paid.on_transfer_result(1, TransferSlot::RoundPayout(0), 1, 1));
        set_caller_at("alice.testnet", 0, 310);
        let consent_after_payout = catch_unwind(AssertUnwindSafe(|| paid.approve_cancellation(1)));
        assert!(consent_after_payout.is_err());
        set_caller_at("anyone.testnet", 0, 311);
        let cancel_after_payout = catch_unwind(AssertUnwindSafe(|| paid.finalize_cancellation(1)));
        assert!(cancel_after_payout.is_err());
        let circle = paid.circles.get(&1).unwrap();
        assert_eq!(circle.successful_payout_count, 1);
        assert_eq!(circle.status, CircleStatus::Active);
    }

    #[test]
    fn payout_success_advances_round_and_completion_enables_collateral_refund() {
        let mut contract = active_contract();
        contribute_both_members(&mut contract, 230);
        set_caller_at("anyone.testnet", 0, AFTER_FIRST_DEADLINE_NS);
        assert!(contract.finalize_payout(1));
        let circle = contract.circles.get(&1).unwrap();
        assert_eq!(circle.rounds[0].status, RoundStatus::PayoutPending);
        assert_eq!(circle.rounds[0].payout.amount_yocto, 2_000);
        assert_eq!(
            circle.rounds[0].payout.recipient,
            TransferRecipient::MemberPosition(1)
        );
        assert_eq!(circle.rounds[0].payout.id, 1);
        assert_eq!(contract.total_pending_outgoing_yocto, 2_000);

        set_callback_result(PromiseResult::Successful(vec![]));
        assert!(contract.on_transfer_result(1, TransferSlot::RoundPayout(0), 1, 1));
        let circle = contract.circles.get(&1).unwrap();
        assert_eq!(circle.successful_payout_count, 1);
        assert_eq!(circle.members[0].total_payouts_yocto, 2_000);
        assert_eq!(circle.current_round, 1);
        assert_eq!(circle.rounds[1].opened_at_ns, 300);
        assert_eq!(circle.rounds[1].deadline_ns, 300 + MIN_PAYMENT_INTERVAL_NS);
        assert_eq!(contract.total_pending_outgoing_yocto, 0);

        contribute_both_members(&mut contract, 330);
        set_caller_at("anyone.testnet", 0, 331);
        assert!(contract.finalize_payout(1));
        set_callback_result(PromiseResult::Successful(vec![]));
        assert!(contract.on_transfer_result(1, TransferSlot::RoundPayout(1), 2, 1));
        let circle = contract.circles.get(&1).unwrap();
        assert_eq!(circle.status, CircleStatus::Completed);
        assert_eq!(circle.successful_payout_count, 2);
        assert_eq!(circle.members[1].total_payouts_yocto, 2_000);

        set_caller_at("alice.testnet", 0, 400);
        assert!(contract.claim_deposit_refund(1));
        let circle = contract.circles.get(&1).unwrap();
        assert_eq!(circle.members[0].collateral_remaining_yocto, 0);
        assert_eq!(circle.members[0].deposit_refund.amount_yocto, 3_000);
        assert_eq!(
            circle.members[0].deposit_refund.recipient,
            TransferRecipient::MemberPosition(1)
        );
        set_callback_result(PromiseResult::Successful(vec![]));
        assert!(contract.on_transfer_result(1, TransferSlot::MemberDepositRefund(1), 3, 1));
        assert_eq!(
            contract.circles.get(&1).unwrap().members[0].status,
            MemberStatus::Refunded
        );
        let duplicate_refund = catch_unwind(AssertUnwindSafe(|| contract.claim_deposit_refund(1)));
        assert!(duplicate_refund.is_err());
    }

    #[test]
    fn failed_payout_retries_same_entitlement_and_rejects_duplicate_or_stale_attempts() {
        let mut contract = active_contract();
        contribute_both_members(&mut contract, 230);
        set_caller_at("anyone.testnet", 0, AFTER_FIRST_DEADLINE_NS);
        contract.finalize_payout(1);
        set_callback_result(PromiseResult::Failed);
        assert!(!contract.on_transfer_result(1, TransferSlot::RoundPayout(0), 1, 1));
        let circle = contract.circles.get(&1).unwrap();
        assert_eq!(circle.rounds[0].status, RoundStatus::TransferFailed);
        assert_eq!(circle.rounds[0].total_collected_yocto, 2_000);
        assert_eq!(circle.rounds[0].payout.state, TransferState::Failed);
        assert_eq!(contract.total_pending_outgoing_yocto, 0);

        set_caller_at("random.testnet", 0, 240);
        assert!(contract.retry_transfer(1, TransferSlot::RoundPayout(0), 1));
        let circle = contract.circles.get(&1).unwrap();
        assert_eq!(circle.rounds[0].status, RoundStatus::PayoutPending);
        assert_eq!(circle.rounds[0].payout.attempts, 2);
        assert_eq!(circle.rounds[0].payout.amount_yocto, 2_000);
        assert_eq!(
            circle.rounds[0].payout.recipient,
            TransferRecipient::MemberPosition(1)
        );
        assert_eq!(contract.total_pending_outgoing_yocto, 2_000);
        let duplicate_retry = catch_unwind(AssertUnwindSafe(|| {
            contract.retry_transfer(1, TransferSlot::RoundPayout(0), 1)
        }));
        assert!(duplicate_retry.is_err());
        set_callback_result(PromiseResult::Successful(vec![]));
        let stale = catch_unwind(AssertUnwindSafe(|| {
            contract.on_transfer_result(1, TransferSlot::RoundPayout(0), 1, 1)
        }));
        assert!(stale.is_err());
        assert_eq!(contract.total_pending_outgoing_yocto, 2_000);
        assert!(contract.on_transfer_result(1, TransferSlot::RoundPayout(0), 1, 2));
        let duplicate = catch_unwind(AssertUnwindSafe(|| {
            contract.on_transfer_result(1, TransferSlot::RoundPayout(0), 1, 2)
        }));
        assert!(duplicate.is_err());
        assert_eq!(contract.total_pending_outgoing_yocto, 0);
        assert_eq!(contract.circles.get(&1).unwrap().successful_payout_count, 1);
    }

    #[test]
    fn unanimous_cancellation_freezes_non_double_counted_refunds() {
        let mut contract = active_contract();
        set_caller_at("alice.testnet", 1_000, 230);
        contract.contribute(1);
        set_caller_at("anyone.testnet", 0, AFTER_FIRST_DEADLINE_NS);
        assert_eq!(contract.process_defaults(1), 1);
        set_caller_at("alice.testnet", 0, 232);
        contract.approve_cancellation(1);
        set_caller_at("bob.testnet", 0, 232);
        contract.approve_cancellation(1);
        set_caller_at("random.testnet", 0, 233);
        assert!(contract.finalize_cancellation(1));

        let circle = contract.circles.get(&1).unwrap();
        assert_eq!(circle.status, CircleStatus::Cancelled);
        assert_eq!(circle.members[0].cancellation_refund_yocto, 4_000);
        assert_eq!(circle.members[1].collateral_remaining_yocto, 0);
        assert_eq!(circle.rounds[0].collateral_substitutions_yocto[1], 1_000);
        assert_eq!(circle.members[1].cancellation_refund_yocto, 3_000);
        assert_eq!(
            circle.members[0].cancellation_refund_yocto
                + circle.members[1].cancellation_refund_yocto,
            7_000
        );

        set_caller_at("anyone.testnet", 0, 234);
        assert!(contract.claim_cancellation_refund(1, 2));
        assert_eq!(
            contract.circles.get(&1).unwrap().members[1].cancellation_refund_yocto,
            0
        );
        let refund = &contract.circles.get(&1).unwrap().members[1].cancellation_refund;
        assert_eq!(refund.amount_yocto, 3_000);
        assert_eq!(refund.recipient, TransferRecipient::MemberPosition(2));
        set_callback_result(PromiseResult::Failed);
        assert!(!contract.on_transfer_result(
            1,
            TransferSlot::MemberCancellationRefund(2),
            refund.id,
            1
        ));
        let refund = &contract.circles.get(&1).unwrap().members[1].cancellation_refund;
        assert_eq!(refund.state, TransferState::Failed);
        set_caller_at("anyone.testnet", 0, 235);
        assert!(contract.retry_transfer(1, TransferSlot::MemberCancellationRefund(2), refund.id));
        let retry = &contract.circles.get(&1).unwrap().members[1].cancellation_refund;
        assert_eq!(retry.amount_yocto, 3_000);
        assert_eq!(retry.recipient, TransferRecipient::MemberPosition(2));
        assert_eq!(retry.attempts, 2);
    }
}
