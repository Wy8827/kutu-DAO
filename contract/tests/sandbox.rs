use anyhow::Result;
use near_workspaces::types::NearToken;
use near_workspaces::{Account, Contract, Worker};
use serde_json::json;

const WASM: &[u8] = include_bytes!("../target/near/kutu_dao_contract.wasm");
const ONE_NEAR: u128 = 10u128.pow(24);

async fn deploy(worker: &Worker<impl near_workspaces::DevNetwork>) -> Result<Contract> {
    let contract = worker.dev_deploy(WASM).await?;
    contract
        .call("new")
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    Ok(contract)
}

async fn create_circle(
    creator: &Account,
    contract: &Contract,
    max_members: u32,
) -> Result<near_workspaces::result::ExecutionFinalResult> {
    Ok(creator
        .call(contract.id(), "create_circle")
        .args_json(json!({
            "args": {
                "name": "Sandbox circle",
                "description": "Actual optimized WASM test",
                "max_members": max_members,
                "contribution_yocto": "1000",
                "security_deposit_yocto": "3000",
                "payment_interval_ns": 60_000_000_000u64
            }
        }))
        .deposit(NearToken::from_yoctonear(ONE_NEAR))
        .max_gas()
        .transact()
        .await?)
}

async fn join(
    member: &Account,
    contract: &Contract,
    circle_id: u64,
) -> Result<near_workspaces::result::ExecutionFinalResult> {
    Ok(member
        .call(contract.id(), "join_circle")
        .args_json(json!({ "circle_id": circle_id }))
        .deposit(NearToken::from_yoctonear(ONE_NEAR))
        .max_gas()
        .transact()
        .await?)
}

#[tokio::test]
async fn optimized_wasm_create_join_rejection_and_max_capacity_measurements() -> Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let contract = deploy(&worker).await?;

    // Duplicate membership is checked in an Open circle so the failure cannot be
    // explained by the circle already being full or Active.
    let duplicate_check_creator = worker.dev_create_account().await?;
    let duplicate_circle = create_circle(&duplicate_check_creator, &contract, 2).await?;
    anyhow::ensure!(
        duplicate_circle.is_success(),
        "duplicate-check circle creation failed"
    );
    let creator_join = join(&duplicate_check_creator, &contract, 1).await?;
    anyhow::ensure!(
        creator_join.is_success(),
        "creator failed to explicitly join"
    );
    let duplicate = join(&duplicate_check_creator, &contract, 1).await?;
    anyhow::ensure!(
        duplicate.is_failure(),
        "duplicate membership unexpectedly succeeded"
    );
    let forged_callback = worker.dev_create_account().await?;
    let forged = forged_callback
        .call(contract.id(), "on_transfer_result")
        .args_json(json!({
            "circle_id": 1,
            "slot": "CircleStorageRefund",
            "transfer_id": 1,
            "attempt": 1
        }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        forged.is_failure(),
        "external account invoked a private callback"
    );

    // A separate maximum-capacity circle measures actual runtime gas and storage.
    let creator = worker.dev_create_account().await?;
    let before_create = worker.view_account(contract.id()).await?.storage_usage;
    let create = create_circle(&creator, &contract, 10).await?;
    anyhow::ensure!(create.is_success(), "10-member circle creation failed");
    let after_create = worker.view_account(contract.id()).await?.storage_usage;
    let create_storage_bytes = after_create.saturating_sub(before_create);
    let create_gas = create.total_gas_burnt.as_gas();

    // The creator is not auto-enrolled; this first position is assigned only
    // after their explicit join call.
    let before_creator_join = worker.view_account(contract.id()).await?.storage_usage;
    let creator_join = join(&creator, &contract, 2).await?;
    anyhow::ensure!(
        creator_join.is_success(),
        "creator failed to explicitly join"
    );
    let after_creator_join = worker.view_account(contract.id()).await?.storage_usage;
    let creator_join_storage_bytes = after_creator_join.saturating_sub(before_creator_join);
    let mut joins_gas_total = creator_join.total_gas_burnt.as_gas();
    let mut max_join_gas = joins_gas_total;
    let mut joins_storage_bytes = creator_join_storage_bytes;
    let mut max_circle_members = vec![creator.clone()];

    for position in 2..=10 {
        let member = worker.dev_create_account().await?;
        let before = worker.view_account(contract.id()).await?.storage_usage;
        let outcome = join(&member, &contract, 2).await?;
        anyhow::ensure!(
            outcome.is_success(),
            "member at position {position} failed to join"
        );
        let after = worker.view_account(contract.id()).await?.storage_usage;
        joins_storage_bytes += after.saturating_sub(before);
        let gas = outcome.total_gas_burnt.as_gas();
        joins_gas_total += gas;
        max_join_gas = max_join_gas.max(gas);
        max_circle_members.push(member);
    }

    for member in &max_circle_members {
        let contribution = member
            .call(contract.id(), "contribute")
            .args_json(json!({ "circle_id": 2 }))
            .deposit(NearToken::from_yoctonear(1_000))
            .max_gas()
            .transact()
            .await?;
        anyhow::ensure!(
            contribution.is_success(),
            "maximum-circle contribution failed"
        );
    }
    let maximum_payout = max_circle_members[0]
        .call(contract.id(), "finalize_payout")
        .args_json(json!({ "circle_id": 2 }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(maximum_payout.is_success(), "maximum-circle payout failed");
    let max_round: serde_json::Value = contract
        .view("get_round")
        .args_json(json!({ "circle_id": 2, "round_number": 0 }))
        .await?
        .json()?;
    anyhow::ensure!(
        max_round["status"] == "Finalized",
        "maximum-circle payout callback failed"
    );

    let full_creator = worker.dev_create_account().await?;
    let full_circle = create_circle(&full_creator, &contract, 10).await?;
    anyhow::ensure!(
        full_circle.is_success(),
        "full-circle test circle creation failed"
    );
    for _ in 0..10 {
        let member = worker.dev_create_account().await?;
        let result = join(&member, &contract, 3).await?;
        anyhow::ensure!(
            result.is_success(),
            "join failed before capacity was reached"
        );
    }
    let eleventh = worker.dev_create_account().await?;
    let full_join = join(&eleventh, &contract, 3).await?;
    anyhow::ensure!(
        full_join.is_failure(),
        "full-circle join unexpectedly succeeded"
    );

    println!(
        "SANDBOX_WASM_METRICS create_storage_bytes={create_storage_bytes} create_gas={create_gas} ten_join_storage_bytes={joins_storage_bytes} ten_join_gas={joins_gas_total} max_individual_join_gas={max_join_gas} duplicate_rejected=true full_rejected=true max_capacity_payout_callback=true"
    );

    // Exercise the first financial round against optimized Wasm and real Sandbox
    // receipts, including the asynchronous payout callback and read-only views.
    let payout_creator = worker.dev_create_account().await?;
    let payout_create = create_circle(&payout_creator, &contract, 2).await?;
    anyhow::ensure!(
        payout_create.is_success(),
        "financial circle creation failed"
    );
    let payout_circle_id = 4u64;
    let first_member = payout_creator;
    let second_member = worker.dev_create_account().await?;
    anyhow::ensure!(
        join(&first_member, &contract, payout_circle_id)
            .await?
            .is_success(),
        "first financial member failed to join"
    );
    anyhow::ensure!(
        join(&second_member, &contract, payout_circle_id)
            .await?
            .is_success(),
        "second financial member failed to join"
    );

    let members: serde_json::Value = contract
        .view("get_members")
        .args_json(json!({ "circle_id": payout_circle_id }))
        .await?
        .json()?;
    anyhow::ensure!(
        members[0]["payout_position"] == 1,
        "first payout position is incorrect"
    );
    anyhow::ensure!(
        members[1]["payout_position"] == 2,
        "second payout position is incorrect"
    );

    let outsider = worker.dev_create_account().await?;
    let unauthorized_contribution = outsider
        .call(contract.id(), "contribute")
        .args_json(json!({ "circle_id": payout_circle_id }))
        .deposit(NearToken::from_yoctonear(1_000))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        unauthorized_contribution.is_failure(),
        "nonmember contribution unexpectedly succeeded"
    );

    for member in [&first_member, &second_member] {
        let contribution = member
            .call(contract.id(), "contribute")
            .args_json(json!({ "circle_id": payout_circle_id }))
            .deposit(NearToken::from_yoctonear(1_000))
            .max_gas()
            .transact()
            .await?;
        anyhow::ensure!(
            contribution.is_success(),
            "exact member contribution failed"
        );
    }
    let first_balance_before = worker
        .view_account(first_member.id())
        .await?
        .balance
        .as_yoctonear();
    let payout = outsider
        .call(contract.id(), "finalize_payout")
        .args_json(json!({ "circle_id": payout_circle_id }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        payout.is_success(),
        "permissionless payout finalization failed"
    );
    let first_balance_after = worker
        .view_account(first_member.id())
        .await?
        .balance
        .as_yoctonear();
    anyhow::ensure!(
        first_balance_after - first_balance_before == 2_000,
        "payout did not reach join-order position 1"
    );
    let round: serde_json::Value = contract
        .view("get_round")
        .args_json(json!({ "circle_id": payout_circle_id, "round_number": 0 }))
        .await?
        .json()?;
    anyhow::ensure!(
        round["status"] == "Finalized",
        "payout callback did not finalize the round"
    );
    println!("SANDBOX_FINANCIAL_LIFECYCLE round0_payout=2000 recipient_position=1 callback_finalized=true");

    // An uncovered member pauses the round. Only that member can cure, after
    // which ordered processing advances to the next overdue member.
    let cure_creator = worker.dev_create_account().await?;
    let cure_create = cure_creator
        .call(contract.id(), "create_circle")
        .args_json(json!({
            "args": {
                "name": "Cure sandbox circle",
                "description": null,
                "max_members": 2,
                "contribution_yocto": "1000",
                "security_deposit_yocto": "500",
                "payment_interval_ns": 60_000_000_000u64
            }
        }))
        .deposit(NearToken::from_yoctonear(ONE_NEAR))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(cure_create.is_success(), "cure circle creation failed");
    let cure_circle_id = 5u64;
    let cure_member_1 = worker.dev_create_account().await?;
    let cure_member_2 = worker.dev_create_account().await?;
    anyhow::ensure!(
        join(&cure_member_1, &contract, cure_circle_id)
            .await?
            .is_success(),
        "first cure member failed to join"
    );
    anyhow::ensure!(
        join(&cure_member_2, &contract, cure_circle_id)
            .await?
            .is_success(),
        "second cure member failed to join"
    );
    worker.fast_forward(1_000).await?;
    let cure_circle_view: serde_json::Value = contract
        .view("get_circle")
        .args_json(json!({ "circle_id": cure_circle_id }))
        .await?
        .json()?;
    let current_block = worker.view_block().await?;
    let first_deadline_ns = cure_circle_view["first_deadline_ns"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("first deadline view is not a string"))?
        .parse::<u64>()?;
    anyhow::ensure!(
        current_block.timestamp() > first_deadline_ns,
        "Sandbox did not advance beyond the first deadline"
    );
    let pause = outsider
        .call(contract.id(), "process_defaults")
        .args_json(json!({ "circle_id": cure_circle_id }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        pause.is_success(),
        "post-deadline pause processing failed: {pause:?}"
    );
    let paused: serde_json::Value = contract
        .view("get_circle")
        .args_json(json!({ "circle_id": cure_circle_id }))
        .await?
        .json()?;
    anyhow::ensure!(
        paused["status"] == "Paused",
        "short collateral did not pause the circle"
    );
    anyhow::ensure!(
        paused["paused_member_position"] == 1,
        "wrong member was selected for cure"
    );
    let wrong_curer = cure_member_2
        .call(contract.id(), "cure_default")
        .args_json(json!({ "circle_id": cure_circle_id }))
        .deposit(NearToken::from_yoctonear(1_000))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        wrong_curer.is_failure(),
        "nondefaulted member cured the pause"
    );
    let short_cure = cure_member_1
        .call(contract.id(), "cure_default")
        .args_json(json!({ "circle_id": cure_circle_id }))
        .deposit(NearToken::from_yoctonear(999))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(short_cure.is_failure(), "short cure unexpectedly succeeded");
    let first_cure = cure_member_1
        .call(contract.id(), "cure_default")
        .args_json(json!({ "circle_id": cure_circle_id }))
        .deposit(NearToken::from_yoctonear(1_000))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(first_cure.is_success(), "exact cure failed");
    let next_default = outsider
        .call(contract.id(), "process_defaults")
        .args_json(json!({ "circle_id": cure_circle_id }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(next_default.is_success(), "next ordered default failed");
    let second_cure = cure_member_2
        .call(contract.id(), "cure_default")
        .args_json(json!({ "circle_id": cure_circle_id }))
        .deposit(NearToken::from_yoctonear(1_000))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(second_cure.is_success(), "second exact cure failed");
    let cured_round: serde_json::Value = contract
        .view("get_round")
        .args_json(json!({ "circle_id": cure_circle_id, "round_number": 0 }))
        .await?
        .json()?;
    anyhow::ensure!(
        cured_round["status"] == "ReadyForPayout",
        "cured round is not ready"
    );

    // Unanimous cancellation freezes the member entitlements, then pays each
    // fixed recipient through the async refund slots.
    let cancel_creator = worker.dev_create_account().await?;
    let cancel_create = create_circle(&cancel_creator, &contract, 2).await?;
    anyhow::ensure!(
        cancel_create.is_success(),
        "cancellation circle creation failed"
    );
    let cancel_circle_id = 6u64;
    let cancel_member = cancel_creator;
    let cancel_other = worker.dev_create_account().await?;
    anyhow::ensure!(
        join(&cancel_member, &contract, cancel_circle_id)
            .await?
            .is_success(),
        "first cancellation member failed to join"
    );
    anyhow::ensure!(
        join(&cancel_other, &contract, cancel_circle_id)
            .await?
            .is_success(),
        "second cancellation member failed to join"
    );
    let cancel_contribution = cancel_member
        .call(contract.id(), "contribute")
        .args_json(json!({ "circle_id": cancel_circle_id }))
        .deposit(NearToken::from_yoctonear(1_000))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        cancel_contribution.is_success(),
        "cancellation example contribution failed"
    );
    worker.fast_forward(1_000).await?;
    let collateral_cover = outsider
        .call(contract.id(), "process_defaults")
        .args_json(json!({ "circle_id": cancel_circle_id }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        collateral_cover.is_success(),
        "cancellation collateral coverage failed"
    );
    for member in [&cancel_member, &cancel_other] {
        let vote = member
            .call(contract.id(), "approve_cancellation")
            .args_json(json!({ "circle_id": cancel_circle_id }))
            .max_gas()
            .transact()
            .await?;
        anyhow::ensure!(vote.is_success(), "member cancellation approval failed");
    }
    let cancel = outsider
        .call(contract.id(), "finalize_cancellation")
        .args_json(json!({ "circle_id": cancel_circle_id }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(cancel.is_success(), "unanimous cancellation failed");
    for position in 1..=2 {
        let claim = outsider
            .call(contract.id(), "claim_cancellation_refund")
            .args_json(json!({ "circle_id": cancel_circle_id, "position": position }))
            .max_gas()
            .transact()
            .await?;
        anyhow::ensure!(
            claim.is_success(),
            "fixed-recipient cancellation refund failed"
        );
    }
    let cancel_members: serde_json::Value = contract
        .view("get_members")
        .args_json(json!({ "circle_id": cancel_circle_id }))
        .await?
        .json()?;
    anyhow::ensure!(
        cancel_members[0]["cancellation_refund"]["state"] == "Succeeded",
        "first cancellation refund callback failed"
    );
    anyhow::ensure!(
        cancel_members[1]["cancellation_refund"]["state"] == "Succeeded",
        "second cancellation refund callback failed"
    );

    // Force a real native transfer receipt failure by paying a member account
    // that is deleted after joining, then recreate the same account before retry.
    let retry_creator = worker.dev_create_account().await?;
    let retry_create = create_circle(&retry_creator, &contract, 2).await?;
    anyhow::ensure!(retry_create.is_success(), "retry circle creation failed");
    let retry_circle_id = 7u64;
    let root = worker.root_account()?;
    let retry_recipient = root
        .create_subaccount("retry-recipient")
        .initial_balance(NearToken::from_near(10))
        .transact()
        .await?
        .into_result()?;
    let retry_contributor = worker.dev_create_account().await?;
    anyhow::ensure!(
        join(&retry_recipient, &contract, retry_circle_id)
            .await?
            .is_success(),
        "retry recipient failed to join at position 1"
    );
    anyhow::ensure!(
        join(&retry_contributor, &contract, retry_circle_id)
            .await?
            .is_success(),
        "retry contributor failed to join at position 2"
    );
    let retry_members: serde_json::Value = contract
        .view("get_members")
        .args_json(json!({ "circle_id": retry_circle_id }))
        .await?
        .json()?;
    anyhow::ensure!(
        retry_members[0]["storage_allowance_refund"]["state"] == "Succeeded",
        "recipient storage refund did not settle before account deletion"
    );
    for member in [&retry_recipient, &retry_contributor] {
        let contribution = member
            .call(contract.id(), "contribute")
            .args_json(json!({ "circle_id": retry_circle_id }))
            .deposit(NearToken::from_yoctonear(1_000))
            .max_gas()
            .transact()
            .await?;
        anyhow::ensure!(
            contribution.is_success(),
            "retry scenario contribution failed"
        );
    }
    let ready_round: serde_json::Value = contract
        .view("get_round")
        .args_json(json!({ "circle_id": retry_circle_id, "round_number": 0 }))
        .await?
        .json()?;
    anyhow::ensure!(
        ready_round["status"] == "ReadyForPayout",
        "retry scenario round was not fully funded"
    );
    let deleted_id = retry_recipient.id().clone();
    let delete_result = retry_recipient.delete_account(root.id()).await?;
    anyhow::ensure!(
        delete_result.is_success(),
        "could not delete the fixed payout recipient"
    );
    anyhow::ensure!(
        worker.view_account(&deleted_id).await.is_err(),
        "payout recipient still exists after deletion"
    );

    // The top-level payout call schedules a real Promise::transfer to a
    // nonexistent named account. The transfer receipt fails; its callback
    // must record Failed and leave the round/circle unadvanced.
    let storage_before_failed_payout = worker.view_account(contract.id()).await?.storage_usage;
    let failed_attempt = outsider
        .call(contract.id(), "finalize_payout")
        .args_json(json!({ "circle_id": retry_circle_id }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        failed_attempt.is_success(),
        "payout initiation/callback transaction itself failed unexpectedly"
    );
    anyhow::ensure!(
        failed_attempt
            .receipt_failures()
            .iter()
            .any(|outcome| outcome.executor_id == deleted_id),
        "Sandbox did not report a failed receipt for the deleted payout recipient"
    );
    let failed_round: serde_json::Value = contract
        .view("get_round")
        .args_json(json!({ "circle_id": retry_circle_id, "round_number": 0 }))
        .await?
        .json()?;
    let failed_circle: serde_json::Value = contract
        .view("get_circle")
        .args_json(json!({ "circle_id": retry_circle_id }))
        .await?
        .json()?;
    let failed_members: serde_json::Value = contract
        .view("get_members")
        .args_json(json!({ "circle_id": retry_circle_id }))
        .await?
        .json()?;
    let payout_id = failed_round["payout"]["id"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("failed payout has no operation ID"))?;
    let failed_event_count = failed_attempt
        .logs()
        .iter()
        .filter(|log| {
            log.contains("\"event\":\"transfer_failed\"")
                && log.contains(&format!("\"transfer_id\":{payout_id}"))
                && log.contains("\"attempt\":1")
        })
        .count();
    anyhow::ensure!(
        failed_event_count == 1,
        "expected exactly one failed callback event for attempt 1"
    );
    anyhow::ensure!(
        failed_round["payout"]["state"] == "Failed"
            && failed_round["payout"]["attempts"] == 1
            && failed_round["payout"]["amount_yocto"] == "2000"
            && failed_round["payout"]["recipient_id"] == deleted_id.as_str(),
        "failed callback did not preserve the original payout operation"
    );
    anyhow::ensure!(
        failed_round["status"] == "TransferFailed"
            && failed_round["total_collected_yocto"] == "2000"
            && failed_circle["current_round"] == 0
            && failed_circle["successful_payout_count"] == 0
            && failed_members[0]["collateral_remaining_yocto"] == "3000",
        "failed payout changed round, payout, or collateral accounting"
    );
    let storage_after_failed_payout = worker.view_account(contract.id()).await?.storage_usage;
    anyhow::ensure!(
        storage_after_failed_payout == storage_before_failed_payout,
        "failed payout callback unexpectedly grew storage"
    );

    // An incorrect ID is permissionless to attempt but cannot authorize a
    // retry. It must leave the Failed claim untouched.
    let unauthorized_retry = outsider
        .call(contract.id(), "retry_transfer")
        .args_json(json!({
            "circle_id": retry_circle_id,
            "slot": { "RoundPayout": 0 },
            "transfer_id": payout_id + 1
        }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        unauthorized_retry.is_failure(),
        "retry with an incorrect operation ID unexpectedly succeeded"
    );
    let after_wrong_retry: serde_json::Value = contract
        .view("get_round")
        .args_json(json!({ "circle_id": retry_circle_id, "round_number": 0 }))
        .await?
        .json()?;
    anyhow::ensure!(
        after_wrong_retry["payout"]["state"] == "Failed"
            && after_wrong_retry["payout"]["attempts"] == 1,
        "invalid retry mutated the saved entitlement"
    );

    // Recreate the exact account ID, then retry permissionlessly with the
    // saved operation ID. The amount and recipient are not retry parameters.
    let restored_recipient = root
        .create_subaccount("retry-recipient")
        .initial_balance(NearToken::from_near(10))
        .transact()
        .await?
        .into_result()?;
    anyhow::ensure!(
        restored_recipient.id() == &deleted_id,
        "recreated recipient ID differs from the immutable saved recipient"
    );
    let recipient_balance_before = worker
        .view_account(&deleted_id)
        .await?
        .balance
        .as_yoctonear();
    let retry = outsider
        .call(contract.id(), "retry_transfer")
        .args_json(json!({
            "circle_id": retry_circle_id,
            "slot": { "RoundPayout": 0 },
            "transfer_id": payout_id
        }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(retry.is_success(), "permissionless payout retry failed");
    let recipient_balance_after = worker
        .view_account(&deleted_id)
        .await?
        .balance
        .as_yoctonear();
    let succeeded_round: serde_json::Value = contract
        .view("get_round")
        .args_json(json!({ "circle_id": retry_circle_id, "round_number": 0 }))
        .await?
        .json()?;
    let succeeded_circle: serde_json::Value = contract
        .view("get_circle")
        .args_json(json!({ "circle_id": retry_circle_id }))
        .await?
        .json()?;
    let succeeded_members: serde_json::Value = contract
        .view("get_members")
        .args_json(json!({ "circle_id": retry_circle_id }))
        .await?
        .json()?;
    anyhow::ensure!(
        recipient_balance_after - recipient_balance_before == 2_000,
        "recipient did not receive exactly one payout on retry"
    );
    let retry_success_event_count = retry
        .logs()
        .iter()
        .filter(|log| {
            log.contains("\"event\":\"transfer_succeeded\"")
                && log.contains(&format!("\"transfer_id\":{payout_id}"))
                && log.contains("\"attempt\":2")
        })
        .count();
    anyhow::ensure!(
        retry_success_event_count == 1,
        "expected exactly one successful callback event for attempt 2"
    );
    anyhow::ensure!(
        succeeded_round["payout"]["state"] == "Succeeded"
            && succeeded_round["payout"]["id"] == payout_id
            && succeeded_round["payout"]["attempts"] == 2
            && succeeded_round["status"] == "Finalized"
            && succeeded_circle["current_round"] == 1
            && succeeded_circle["successful_payout_count"] == 1,
        "retry did not settle and advance exactly once"
    );
    anyhow::ensure!(
        succeeded_members[0]["collateral_remaining_yocto"] == "3000"
            && succeeded_members[0]["total_payouts_yocto"] == "2000",
        "payout retry changed collateral or recorded the wrong payout total"
    );
    let duplicate_retry = outsider
        .call(contract.id(), "retry_transfer")
        .args_json(json!({
            "circle_id": retry_circle_id,
            "slot": { "RoundPayout": 0 },
            "transfer_id": payout_id
        }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        duplicate_retry.is_failure(),
        "retry after successful settlement unexpectedly succeeded"
    );
    let recipient_balance_final = worker
        .view_account(&deleted_id)
        .await?
        .balance
        .as_yoctonear();
    anyhow::ensure!(
        recipient_balance_final == recipient_balance_after,
        "duplicate retry produced a second payment"
    );
    let storage_after_retry = worker.view_account(contract.id()).await?.storage_usage;
    anyhow::ensure!(
        storage_after_retry == storage_after_failed_payout,
        "payout retry unexpectedly grew storage"
    );
    println!(
        "SANDBOX_FAILED_TRANSFER_RETRY transfer_id={payout_id} failed_attempt=1 retry_attempt=2 amount_yocto=2000 recipient_balance_delta=2000 failed_attempt_gas={} retry_gas={} storage_delta_failed={} storage_delta_retry={} round_advanced_once=true duplicate_retry_rejected=true",
        failed_attempt.total_gas_burnt.as_gas(),
        retry.total_gas_burnt.as_gas(),
        storage_after_failed_payout.saturating_sub(storage_before_failed_payout),
        storage_after_retry.saturating_sub(storage_after_failed_payout)
    );

    // Exercise a real failed member-deposit refund receipt. Submit the claim
    // asynchronously, then delete the recipient before waiting for its promise
    // chain. The saved refund must remain retryable for the same account ID.
    let refund_creator = worker.dev_create_account().await?;
    let refund_create = create_circle(&refund_creator, &contract, 2).await?;
    anyhow::ensure!(refund_create.is_success(), "refund circle creation failed");
    let refund_circle_id = 8u64;
    let refund_recipient = root
        .create_subaccount("refund-recipient")
        .initial_balance(NearToken::from_near(10))
        .transact()
        .await?
        .into_result()?;
    let refund_contributor = worker.dev_create_account().await?;
    anyhow::ensure!(
        join(&refund_recipient, &contract, refund_circle_id)
            .await?
            .is_success(),
        "refund recipient failed to join"
    );
    anyhow::ensure!(
        join(&refund_contributor, &contract, refund_circle_id)
            .await?
            .is_success(),
        "refund contributor failed to join"
    );
    for round_number in 0..2u32 {
        for member in [&refund_recipient, &refund_contributor] {
            let contribution = member
                .call(contract.id(), "contribute")
                .args_json(json!({ "circle_id": refund_circle_id }))
                .deposit(NearToken::from_yoctonear(1_000))
                .max_gas()
                .transact()
                .await?;
            anyhow::ensure!(
                contribution.is_success(),
                "deposit-refund scenario contribution failed"
            );
        }
        let payout = outsider
            .call(contract.id(), "finalize_payout")
            .args_json(json!({ "circle_id": refund_circle_id }))
            .max_gas()
            .transact()
            .await?;
        anyhow::ensure!(payout.is_success(), "deposit-refund scenario payout failed");
        let completed_round: serde_json::Value = contract
            .view("get_round")
            .args_json(json!({
                "circle_id": refund_circle_id,
                "round_number": round_number
            }))
            .await?
            .json()?;
        anyhow::ensure!(
            completed_round["status"] == "Finalized",
            "deposit-refund setup round did not finalize"
        );
    }
    let completed_refund_circle: serde_json::Value = contract
        .view("get_circle")
        .args_json(json!({ "circle_id": refund_circle_id }))
        .await?
        .json()?;
    anyhow::ensure!(
        completed_refund_circle["status"] == "Completed",
        "deposit-refund setup circle did not complete"
    );

    let refund_recipient_id = refund_recipient.id().clone();
    let refund_claim_status = refund_recipient
        .call(contract.id(), "claim_deposit_refund")
        .args_json(json!({ "circle_id": refund_circle_id }))
        .max_gas()
        .transact_async()
        .await?;
    let delete_refund_recipient = refund_recipient.delete_account(root.id()).await?;
    anyhow::ensure!(
        delete_refund_recipient.is_success(),
        "could not delete refund recipient between claim and transfer receipt"
    );
    let failed_refund = refund_claim_status.await?;
    anyhow::ensure!(
        failed_refund
            .receipt_failures()
            .iter()
            .any(|outcome| outcome.executor_id == refund_recipient_id),
        "Sandbox did not report a failed member deposit-refund receipt"
    );
    let failed_refund_members: serde_json::Value = contract
        .view("get_members")
        .args_json(json!({ "circle_id": refund_circle_id }))
        .await?
        .json()?;
    let refund_transfer_id = failed_refund_members[0]["deposit_refund"]["id"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("failed deposit refund has no operation ID"))?;
    anyhow::ensure!(
        failed_refund_members[0]["deposit_refund"]["state"] == "Failed"
            && failed_refund_members[0]["deposit_refund"]["amount_yocto"] == "3000"
            && failed_refund_members[0]["deposit_refund"]["attempts"] == 1
            && failed_refund_members[0]["collateral_remaining_yocto"] == "0",
        "failed deposit refund did not preserve the exact collateral entitlement"
    );
    let restore_refund_recipient = root
        .create_subaccount("refund-recipient")
        .initial_balance(NearToken::from_near(10))
        .transact()
        .await?
        .into_result()?;
    anyhow::ensure!(
        restore_refund_recipient.id() == &refund_recipient_id,
        "recreated refund recipient has a different account ID"
    );
    let refund_balance_before_retry = worker
        .view_account(&refund_recipient_id)
        .await?
        .balance
        .as_yoctonear();
    let refund_retry = outsider
        .call(contract.id(), "retry_transfer")
        .args_json(json!({
            "circle_id": refund_circle_id,
            "slot": { "MemberDepositRefund": 1 },
            "transfer_id": refund_transfer_id
        }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(refund_retry.is_success(), "deposit refund retry failed");
    let refund_balance_after_retry = worker
        .view_account(&refund_recipient_id)
        .await?
        .balance
        .as_yoctonear();
    let retried_refund_members: serde_json::Value = contract
        .view("get_members")
        .args_json(json!({ "circle_id": refund_circle_id }))
        .await?
        .json()?;
    anyhow::ensure!(
        refund_balance_after_retry - refund_balance_before_retry == 3_000
            && retried_refund_members[0]["deposit_refund"]["id"] == refund_transfer_id
            && retried_refund_members[0]["deposit_refund"]["attempts"] == 2
            && retried_refund_members[0]["deposit_refund"]["state"] == "Succeeded"
            && retried_refund_members[0]["status"] == "Refunded",
        "deposit refund retry did not pay exactly once to its saved recipient"
    );
    let duplicate_refund_retry = outsider
        .call(contract.id(), "retry_transfer")
        .args_json(json!({
            "circle_id": refund_circle_id,
            "slot": { "MemberDepositRefund": 1 },
            "transfer_id": refund_transfer_id
        }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        duplicate_refund_retry.is_failure(),
        "successful deposit refund was accepted for a duplicate retry"
    );
    println!(
        "SANDBOX_FAILED_DEPOSIT_REFUND_RETRY transfer_id={refund_transfer_id} failed_attempt=1 retry_attempt=2 amount_yocto=3000 recipient_balance_delta=3000 duplicate_retry_rejected=true"
    );

    // Cover a collateral substitution followed by a later uncovered member in
    // the same defaults call. Both the applied-collateral and pause events must
    // be emitted so event-driven clients do not miss the partial transition.
    let collateral_event_creator = worker.dev_create_account().await?;
    let collateral_event_create = collateral_event_creator
        .call(contract.id(), "create_circle")
        .args_json(json!({
            "args": {
                "name": "Collateral event sandbox circle",
                "description": null,
                "max_members": 3,
                "contribution_yocto": "1000",
                "security_deposit_yocto": "1000",
                "payment_interval_ns": 60_000_000_000u64
            }
        }))
        .deposit(NearToken::from_near(1))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        collateral_event_create.is_success(),
        "collateral-event circle creation failed"
    );
    let collateral_event_circle_id = 9u64;
    let collateral_event_members = [
        worker.dev_create_account().await?,
        worker.dev_create_account().await?,
        worker.dev_create_account().await?,
    ];
    for member in &collateral_event_members {
        let join_result = member
            .call(contract.id(), "join_circle")
            .args_json(json!({ "circle_id": collateral_event_circle_id }))
            .deposit(NearToken::from_near(1))
            .max_gas()
            .transact()
            .await?;
        anyhow::ensure!(
            join_result.is_success(),
            "collateral-event member join failed"
        );
    }
    for member in &collateral_event_members[..2] {
        let contribution = member
            .call(contract.id(), "contribute")
            .args_json(json!({ "circle_id": collateral_event_circle_id }))
            .deposit(NearToken::from_yoctonear(1_000))
            .max_gas()
            .transact()
            .await?;
        anyhow::ensure!(
            contribution.is_success(),
            "first-round collateral-event contribution failed"
        );
    }
    worker.fast_forward(1_000).await?;
    let first_cover = outsider
        .call(contract.id(), "process_defaults")
        .args_json(json!({ "circle_id": collateral_event_circle_id }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        first_cover.is_success(),
        "first-round collateral substitution failed"
    );
    let first_collateral_events = first_cover
        .logs()
        .iter()
        .filter(|log| {
            log.contains("\"event\":\"default_collateral_applied\"")
                && log.contains("\"member_position\":3")
        })
        .count();
    anyhow::ensure!(
        first_collateral_events == 1,
        "first collateral substitution event missing"
    );
    let collateral_event_payout = outsider
        .call(contract.id(), "finalize_payout")
        .args_json(json!({ "circle_id": collateral_event_circle_id }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        collateral_event_payout.is_success(),
        "collateral-event circle first payout failed"
    );
    let next_contribution = collateral_event_members[0]
        .call(contract.id(), "contribute")
        .args_json(json!({ "circle_id": collateral_event_circle_id }))
        .deposit(NearToken::from_yoctonear(1_000))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        next_contribution.is_success(),
        "second-round collateral-event contribution failed"
    );
    worker.fast_forward(1_000).await?;
    let partial_cover_then_pause = outsider
        .call(contract.id(), "process_defaults")
        .args_json(json!({ "circle_id": collateral_event_circle_id }))
        .max_gas()
        .transact()
        .await?;
    anyhow::ensure!(
        partial_cover_then_pause.is_success(),
        "partial collateral cover followed by pause failed"
    );
    let applied_before_pause = partial_cover_then_pause
        .logs()
        .iter()
        .filter(|log| {
            log.contains("\"event\":\"default_collateral_applied\"")
                && log.contains("\"round_number\":1")
                && log.contains("\"member_position\":2")
        })
        .count();
    let pause_event = partial_cover_then_pause.logs().iter().any(|log| {
        log.contains("\"event\":\"circle_paused\"") && log.contains("\"member_position\":3")
    });
    anyhow::ensure!(
        applied_before_pause == 1 && pause_event,
        "partial collateral event was omitted when processing then paused"
    );
    println!("SANDBOX_COLLATERAL_EVENT partial_coverage_event=true subsequent_pause_event=true");

    Ok(())
}
