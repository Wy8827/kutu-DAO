use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::AccountId;

pub const MAX_MEMBERS: u32 = 10;
pub const MIN_MEMBERS: u32 = 2;
pub const MAX_NAME_BYTES: usize = 64;
pub const MAX_DESCRIPTION_BYTES: usize = 280;
pub const MIN_PAYMENT_INTERVAL_NS: u64 = 60_000_000_000;
pub const MAX_PAYMENT_INTERVAL_NS: u64 = 31_536_000_000_000_000;

#[derive(BorshDeserialize, BorshSerialize, Clone, Debug, PartialEq, Eq)]
pub enum CircleStatus {
    Open,
    Active,
    Paused,
    Completed,
    Cancelled,
    ClockExhausted,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Debug, PartialEq, Eq)]
pub enum MemberStatus {
    Joined,
    RefundPending,
    Refunded,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Debug, PartialEq, Eq)]
pub enum RoundStatus {
    Collecting,
    ReadyForPayout,
    PayoutPending,
    TransferFailed,
    Finalized,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Debug, PartialEq, Eq)]
pub enum DefaultResolution {
    NotProcessed,
    AwaitingCure,
    Cured,
    CollateralSubstituted,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Debug, PartialEq, Eq)]
pub enum TransferKind {
    Payout,
    DepositRefund,
    StorageAllowanceRefund,
    CancellationRefund,
}

/// A stable reference into circle state. Member positions never change after assignment.
#[derive(BorshDeserialize, BorshSerialize, Clone, Debug, PartialEq, Eq)]
pub enum TransferRecipient {
    Creator,
    MemberPosition(u32),
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Debug, PartialEq, Eq)]
pub enum TransferState {
    NotStarted,
    Pending,
    Succeeded,
    Failed,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Debug, PartialEq, Eq)]
pub struct PendingTransfer {
    pub id: u64,
    pub kind: TransferKind,
    pub recipient: TransferRecipient,
    pub amount_yocto: u128,
    pub state: TransferState,
    pub attempts: u32,
}

impl PendingTransfer {
    pub fn new_slot(kind: TransferKind, recipient: TransferRecipient) -> Self {
        Self {
            id: 0,
            kind,
            recipient,
            amount_yocto: 0,
            state: TransferState::NotStarted,
            attempts: 0,
        }
    }

    /// Locks transfer identity and value before a promise is scheduled.
    pub fn begin(&mut self, id: u64, amount_yocto: u128) -> Result<u32, TransferError> {
        if self.state != TransferState::NotStarted || id == 0 || amount_yocto == 0 {
            return Err(TransferError::InvalidStart);
        }
        self.id = id;
        self.amount_yocto = amount_yocto;
        self.state = TransferState::Pending;
        self.attempts = 1;
        Ok(self.attempts)
    }

    pub fn retry(&mut self, id: u64) -> Result<u32, TransferError> {
        if self.id != id || self.state != TransferState::Failed {
            return Err(TransferError::NotRetryable);
        }
        let next_attempt = self
            .attempts
            .checked_add(1)
            .ok_or(TransferError::Overflow)?;
        self.attempts = next_attempt;
        self.state = TransferState::Pending;
        Ok(self.attempts)
    }

    pub fn settle(&mut self, id: u64, attempt: u32, succeeded: bool) -> Result<(), TransferError> {
        if self.id != id || self.attempts != attempt || self.state != TransferState::Pending {
            return Err(TransferError::StaleCallback);
        }
        self.state = if succeeded {
            TransferState::Succeeded
        } else {
            TransferState::Failed
        };
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferError {
    InvalidStart,
    NotRetryable,
    StaleCallback,
    Overflow,
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub account_id: AccountId,
    pub payout_position: u32,
    pub collateral_remaining_yocto: u128,
    pub storage_bytes_added: u64,
    pub storage_cost_paid_yocto: u128,
    pub total_contributions_yocto: u128,
    pub total_payouts_yocto: u128,
    pub on_time_payments: u32,
    pub late_payments: u32,
    pub status: MemberStatus,
    pub cancellation_approved: bool,
    pub cancellation_refund_yocto: u128,
    pub deposit_refund: PendingTransfer,
    pub cancellation_refund: PendingTransfer,
    pub storage_allowance_refund: PendingTransfer,
}

impl Member {
    pub fn new(
        account_id: AccountId,
        payout_position: u32,
        collateral_yocto: u128,
        storage_bytes_added: u64,
        storage_cost_paid_yocto: u128,
    ) -> Self {
        let recipient = TransferRecipient::MemberPosition(payout_position);
        Self {
            account_id,
            payout_position,
            collateral_remaining_yocto: collateral_yocto,
            storage_bytes_added,
            storage_cost_paid_yocto,
            total_contributions_yocto: 0,
            total_payouts_yocto: 0,
            on_time_payments: 0,
            late_payments: 0,
            status: MemberStatus::Joined,
            cancellation_approved: false,
            cancellation_refund_yocto: 0,
            deposit_refund: PendingTransfer::new_slot(
                TransferKind::DepositRefund,
                recipient.clone(),
            ),
            cancellation_refund: PendingTransfer::new_slot(
                TransferKind::CancellationRefund,
                recipient.clone(),
            ),
            storage_allowance_refund: PendingTransfer::new_slot(
                TransferKind::StorageAllowanceRefund,
                recipient,
            ),
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Debug, PartialEq, Eq)]
pub struct Round {
    pub number: u32,
    pub recipient_position: u32,
    pub opened_at_ns: u64,
    pub deadline_ns: u64,
    pub status: RoundStatus,
    /// Fixed-size slot vector indexed by immutable one-based payout position minus one.
    pub direct_payments_yocto: Vec<u128>,
    /// Fixed-size slot vector; a nonzero value is reclassified from that member's collateral.
    pub collateral_substitutions_yocto: Vec<u128>,
    pub default_resolution: Vec<DefaultResolution>,
    pub total_collected_yocto: u128,
    pub payout: PendingTransfer,
    pub finalized_at_ns: u64,
}

impl Round {
    pub fn new(number: u32, member_capacity: u32) -> Self {
        let capacity = usize::try_from(member_capacity).expect("member capacity exceeds usize");
        let recipient_position = number.checked_add(1).expect("round position overflow");
        Self {
            number,
            recipient_position,
            opened_at_ns: 0,
            deadline_ns: 0,
            status: RoundStatus::Collecting,
            direct_payments_yocto: vec![0; capacity],
            collateral_substitutions_yocto: vec![0; capacity],
            default_resolution: vec![DefaultResolution::NotProcessed; capacity],
            total_collected_yocto: 0,
            payout: PendingTransfer::new_slot(
                TransferKind::Payout,
                TransferRecipient::MemberPosition(recipient_position),
            ),
            finalized_at_ns: 0,
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize, Clone, Debug, PartialEq, Eq)]
pub struct Circle {
    pub id: u64,
    pub creator: AccountId,
    pub name: String,
    pub description: Option<String>,
    /// The configured membership target. The protocol maximum remains MAX_MEMBERS.
    pub max_members: u32,
    pub contribution_yocto: u128,
    pub security_deposit_yocto: u128,
    pub payment_interval_ns: u64,
    pub activated_at_ns: u64,
    pub status: CircleStatus,
    /// Only successful joins appear here; creator is included only after joining.
    pub members: Vec<Member>,
    pub rounds: Vec<Round>,
    pub storage_bytes_funded: u64,
    pub storage_cost_funded_yocto: u128,
    pub storage_allowance_refund: PendingTransfer,
    pub current_round: u32,
    pub paused_round: u32,
    pub paused_member_position: u32,
    pub successful_payout_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CircleConfig {
    pub name: String,
    pub description: Option<String>,
    pub max_members: u32,
    pub contribution_yocto: u128,
    pub security_deposit_yocto: u128,
    pub payment_interval_ns: u64,
}

impl Circle {
    pub fn open(id: u64, creator: AccountId, config: CircleConfig) -> Result<Self, CircleError> {
        if !(MIN_PAYMENT_INTERVAL_NS..=MAX_PAYMENT_INTERVAL_NS)
            .contains(&config.payment_interval_ns)
        {
            return Err(CircleError::InvalidPaymentInterval);
        }
        if id == 0
            || config.max_members < MIN_MEMBERS
            || config.max_members > MAX_MEMBERS
            || config.name.as_bytes().len() > MAX_NAME_BYTES
            || config
                .description
                .as_ref()
                .is_some_and(|s| s.as_bytes().len() > MAX_DESCRIPTION_BYTES)
            || config.contribution_yocto == 0
            || config.security_deposit_yocto == 0
            || config.payment_interval_ns == 0
        {
            return Err(CircleError::InvalidConfiguration);
        }
        config
            .contribution_yocto
            .checked_mul(u128::from(config.max_members))
            .ok_or(CircleError::ContributionTotalOverflow)?;
        let round_count =
            usize::try_from(config.max_members).map_err(|_| CircleError::CapacityOverflow)?;
        let mut rounds = Vec::new();
        rounds
            .try_reserve_exact(round_count)
            .map_err(|_| CircleError::CapacityOverflow)?;
        for round in 0..config.max_members {
            rounds.push(Round::new(round, config.max_members));
        }

        Ok(Self {
            id,
            creator: creator.clone(),
            name: config.name,
            description: config.description,
            max_members: config.max_members,
            contribution_yocto: config.contribution_yocto,
            security_deposit_yocto: config.security_deposit_yocto,
            payment_interval_ns: config.payment_interval_ns,
            activated_at_ns: 0,
            status: CircleStatus::Open,
            members: Vec::new(),
            rounds,
            storage_bytes_funded: 0,
            storage_cost_funded_yocto: 0,
            storage_allowance_refund: PendingTransfer::new_slot(
                TransferKind::StorageAllowanceRefund,
                TransferRecipient::Creator,
            ),
            current_round: 0,
            paused_round: 0,
            paused_member_position: 0,
            successful_payout_count: 0,
        })
    }

    /// Applies the membership transition after the payable wrapper verifies funding.
    pub fn append_member(
        &mut self,
        account_id: AccountId,
        collateral_yocto: u128,
        storage_bytes_added: u64,
        storage_cost_paid_yocto: u128,
        joined_at_ns: u64,
    ) -> Result<u32, CircleError> {
        if self.status != CircleStatus::Open {
            return Err(CircleError::NotOpen);
        }
        if self.members.len()
            >= usize::try_from(self.max_members).map_err(|_| CircleError::CapacityOverflow)?
        {
            return Err(CircleError::Full);
        }
        if self
            .members
            .iter()
            .any(|member| member.account_id == account_id)
        {
            return Err(CircleError::DuplicateMember);
        }
        if collateral_yocto != self.security_deposit_yocto {
            return Err(CircleError::IncorrectCollateral);
        }
        let position = u32::try_from(self.members.len())
            .ok()
            .and_then(|length| length.checked_add(1))
            .ok_or(CircleError::CapacityOverflow)?;
        let storage_bytes_funded = self
            .storage_bytes_funded
            .checked_add(storage_bytes_added)
            .ok_or(CircleError::Overflow)?;
        let storage_cost_funded_yocto = self
            .storage_cost_funded_yocto
            .checked_add(storage_cost_paid_yocto)
            .ok_or(CircleError::Overflow)?;
        let activates = position == self.max_members;
        let first_deadline = if activates {
            Some(
                joined_at_ns
                    .checked_add(self.payment_interval_ns)
                    .ok_or(CircleError::DeadlineOverflow)?,
            )
        } else {
            None
        };

        self.members.push(Member::new(
            account_id,
            position,
            collateral_yocto,
            storage_bytes_added,
            storage_cost_paid_yocto,
        ));
        self.storage_bytes_funded = storage_bytes_funded;
        self.storage_cost_funded_yocto = storage_cost_funded_yocto;
        if let Some(deadline) = first_deadline {
            self.status = CircleStatus::Active;
            self.activated_at_ns = joined_at_ns;
            self.rounds[0].opened_at_ns = joined_at_ns;
            self.rounds[0].deadline_ns = deadline;
        }
        Ok(position)
    }

    pub fn record_contribution(
        &mut self,
        member_position: u32,
        attached_yocto: u128,
        now_ns: u64,
    ) -> Result<(), CircleError> {
        if self.status != CircleStatus::Active {
            return Err(CircleError::CircleNotActive);
        }
        let index = self.current_round as usize;
        let round = self.rounds.get(index).ok_or(CircleError::RoundNotFound)?;
        if round.status != RoundStatus::Collecting {
            return Err(CircleError::RoundNotCollecting);
        }
        if now_ns > round.deadline_ns {
            return Err(CircleError::DeadlinePassed);
        }
        if attached_yocto != self.contribution_yocto {
            return Err(CircleError::IncorrectContribution);
        }
        let member_index = member_position
            .checked_sub(1)
            .ok_or(CircleError::MemberNotFound)? as usize;
        if member_index >= self.members.len()
            || member_position as usize > self.max_members as usize
        {
            return Err(CircleError::MemberNotFound);
        }
        let round = &self.rounds[index];
        if round.direct_payments_yocto[member_index] != 0
            || round.collateral_substitutions_yocto[member_index] != 0
            || round.default_resolution[member_index] != DefaultResolution::NotProcessed
        {
            return Err(CircleError::AlreadyFunded);
        }
        let new_total = round
            .total_collected_yocto
            .checked_add(attached_yocto)
            .ok_or(CircleError::Overflow)?;
        let new_member_total = self.members[member_index]
            .total_contributions_yocto
            .checked_add(attached_yocto)
            .ok_or(CircleError::Overflow)?;
        let new_on_time = self.members[member_index]
            .on_time_payments
            .checked_add(1)
            .ok_or(CircleError::Overflow)?;
        let full_amount = self
            .contribution_yocto
            .checked_mul(u128::from(self.max_members))
            .ok_or(CircleError::Overflow)?;

        let round = &mut self.rounds[index];
        round.direct_payments_yocto[member_index] = attached_yocto;
        round.total_collected_yocto = new_total;
        if new_total == full_amount {
            round.status = RoundStatus::ReadyForPayout;
        }
        let member = &mut self.members[member_index];
        member.total_contributions_yocto = new_member_total;
        member.on_time_payments = new_on_time;
        Ok(())
    }

    /// Processes overdue missing slots in immutable join order. Stops at one cure target.
    pub fn process_defaults(&mut self, now_ns: u64) -> Result<u32, CircleError> {
        if self.status != CircleStatus::Active {
            return Err(CircleError::CircleNotActive);
        }
        let round_index = self.current_round as usize;
        let round = self
            .rounds
            .get(round_index)
            .ok_or(CircleError::RoundNotFound)?;
        if round.status != RoundStatus::Collecting {
            return Err(CircleError::RoundNotCollecting);
        }
        if now_ns <= round.deadline_ns {
            return Err(CircleError::DeadlineNotPassed);
        }
        let full_amount = self
            .contribution_yocto
            .checked_mul(u128::from(self.max_members))
            .ok_or(CircleError::Overflow)?;
        let mut covered = 0u32;
        for member_index in 0..self.members.len() {
            let round = &self.rounds[round_index];
            if round.direct_payments_yocto[member_index] != 0
                || round.collateral_substitutions_yocto[member_index] != 0
            {
                continue;
            }
            if round.default_resolution[member_index] != DefaultResolution::NotProcessed {
                return Err(CircleError::InvalidDefaultState);
            }
            if self.members[member_index].collateral_remaining_yocto < self.contribution_yocto {
                let member_position = self.members[member_index].payout_position;
                let paused_round = self
                    .current_round
                    .checked_add(1)
                    .ok_or(CircleError::Overflow)?;
                let round = &mut self.rounds[round_index];
                round.default_resolution[member_index] = DefaultResolution::AwaitingCure;
                round.status = RoundStatus::Collecting;
                self.status = CircleStatus::Paused;
                self.paused_round = paused_round;
                self.paused_member_position = member_position;
                return Ok(covered);
            }
            let new_total = self.rounds[round_index]
                .total_collected_yocto
                .checked_add(self.contribution_yocto)
                .ok_or(CircleError::Overflow)?;
            let new_member_total = self.members[member_index]
                .total_contributions_yocto
                .checked_add(self.contribution_yocto)
                .ok_or(CircleError::Overflow)?;
            let new_late_payments = self.members[member_index]
                .late_payments
                .checked_add(1)
                .ok_or(CircleError::Overflow)?;
            self.members[member_index].collateral_remaining_yocto -= self.contribution_yocto;
            self.members[member_index].total_contributions_yocto = new_member_total;
            self.members[member_index].late_payments = new_late_payments;
            let round = &mut self.rounds[round_index];
            round.collateral_substitutions_yocto[member_index] = self.contribution_yocto;
            round.default_resolution[member_index] = DefaultResolution::CollateralSubstituted;
            round.total_collected_yocto = new_total;
            covered = covered.checked_add(1).ok_or(CircleError::Overflow)?;
        }
        if self.rounds[round_index].total_collected_yocto == full_amount {
            self.rounds[round_index].status = RoundStatus::ReadyForPayout;
        }
        Ok(covered)
    }

    pub fn cure_default(
        &mut self,
        caller: &AccountId,
        attached_yocto: u128,
    ) -> Result<u32, CircleError> {
        if self.status != CircleStatus::Paused {
            return Err(CircleError::CircleNotPaused);
        }
        if attached_yocto != self.contribution_yocto {
            return Err(CircleError::IncorrectContribution);
        }
        if self.paused_round
            != self
                .current_round
                .checked_add(1)
                .ok_or(CircleError::Overflow)?
        {
            return Err(CircleError::InvalidDefaultState);
        }
        let member_position = self.paused_member_position;
        let member_index = member_position
            .checked_sub(1)
            .ok_or(CircleError::MemberNotFound)? as usize;
        if self.members.get(member_index).map(|m| &m.account_id) != Some(caller) {
            return Err(CircleError::Unauthorized);
        }
        let round_index = self.current_round as usize;
        let round = self
            .rounds
            .get(round_index)
            .ok_or(CircleError::RoundNotFound)?;
        if round.default_resolution[member_index] != DefaultResolution::AwaitingCure
            || round.direct_payments_yocto[member_index] != 0
            || round.collateral_substitutions_yocto[member_index] != 0
        {
            return Err(CircleError::InvalidDefaultState);
        }
        let new_total = round
            .total_collected_yocto
            .checked_add(attached_yocto)
            .ok_or(CircleError::Overflow)?;
        let new_member_total = self.members[member_index]
            .total_contributions_yocto
            .checked_add(attached_yocto)
            .ok_or(CircleError::Overflow)?;
        let new_late_payments = self.members[member_index]
            .late_payments
            .checked_add(1)
            .ok_or(CircleError::Overflow)?;
        let full_amount = self
            .contribution_yocto
            .checked_mul(u128::from(self.max_members))
            .ok_or(CircleError::Overflow)?;

        let round = &mut self.rounds[round_index];
        round.direct_payments_yocto[member_index] = attached_yocto;
        round.default_resolution[member_index] = DefaultResolution::Cured;
        round.total_collected_yocto = new_total;
        if new_total == full_amount {
            round.status = RoundStatus::ReadyForPayout;
        }
        let member = &mut self.members[member_index];
        member.total_contributions_yocto = new_member_total;
        member.late_payments = new_late_payments;
        self.status = CircleStatus::Active;
        self.paused_round = 0;
        self.paused_member_position = 0;
        Ok(member_position)
    }

    pub fn cancellation_refund_for(&self, member_index: usize) -> Result<u128, CircleError> {
        let member = self
            .members
            .get(member_index)
            .ok_or(CircleError::MemberNotFound)?;
        let mut refund = member.collateral_remaining_yocto;
        for round in &self.rounds {
            refund = refund
                .checked_add(round.direct_payments_yocto[member_index])
                .and_then(|value| {
                    value.checked_add(round.collateral_substitutions_yocto[member_index])
                })
                .ok_or(CircleError::Overflow)?;
        }
        Ok(refund)
    }

    pub fn has_pending_payout(&self) -> bool {
        self.rounds
            .iter()
            .any(|round| round.payout.state == TransferState::Pending)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CircleError {
    InvalidConfiguration,
    InvalidPaymentInterval,
    ContributionTotalOverflow,
    TextTooLong,
    CapacityOverflow,
    DeadlineOverflow,
    NotOpen,
    Full,
    DuplicateMember,
    IncorrectCollateral,
    IncorrectContribution,
    CircleNotActive,
    CircleNotPaused,
    RoundNotFound,
    RoundNotCollecting,
    DeadlinePassed,
    DeadlineNotPassed,
    MemberNotFound,
    AlreadyFunded,
    InvalidDefaultState,
    Unauthorized,
    NotReady,
    PayoutAlreadySucceeded,
    PayoutPending,
    NotCompleted,
    NotCancelled,
    NotUnanimous,
    AlreadyCancelled,
    AlreadyRefunded,
    NoRefundDue,
    Overflow,
}
