//! Probe tests for suspected library defects found while reading `../taixu`.
//! These are deliberately independent of the demo game.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use futures::executor::block_on;
use taixu::npc::{NpcAction, NpcAgent, NpcContext, NpcDecision, NpcError, NpcRuntime};
use taixu::prelude::*;
use taixu::wallet::Wallet;

/// BUG 1: a resource with a very high regen rate can be inflated by reading
/// the balance repeatedly at the same timestamp.
#[test]
fn probe_resource_regen_repeated_reads() {
    let def = ResourceDef::new("plasma", "Plasma")
        .with_cap(1_000_000)
        .with_regen_per_hour(10_000_000);
    let mut wallet = Wallet::new();
    wallet.set_resource(&def, 0, 0);

    let first = wallet.resource_balance(&def, 1);
    let second = wallet.resource_balance(&def, 1);
    let third = wallet.resource_balance(&def, 1);

    assert_eq!(
        first, second,
        "reading the same timestamp twice must not grant more resource \
         (first={first}, second={second}, third={third})"
    );
}

/// BUG 2: NpcRuntime documents that structured actions are applied atomically,
/// but a failure in a later action leaves earlier actions committed.
#[derive(Debug)]
struct GreedyAgent;

#[async_trait]
impl NpcAgent for GreedyAgent {
    async fn respond(&self, ctx: &NpcContext) -> std::result::Result<NpcDecision, NpcError> {
        Ok(NpcDecision {
            reply: Some("here you go".into()),
            actions: vec![
                NpcAction::GiveReward {
                    account: ctx.speaker.clone(),
                    reward: Reward::new().currency("gold", 100),
                },
                // No such leaderboard: this action must fail.
                NpcAction::SetScore {
                    leaderboard: "does_not_exist".into(),
                    account: ctx.speaker.clone(),
                    score: 1,
                },
            ],
        })
    }
}

#[test]
fn probe_npc_actions_are_atomic() {
    let game = Game::new();
    game.create_region("asia", "Asia", None, None).unwrap();
    let alice = game.create_account("alice", "Alice", "asia").unwrap();
    game.register_npc(NpcDef::new("guide", "Guide", "test"))
        .unwrap();

    let mut runtime = NpcRuntime::new();
    runtime.register("guide", Arc::new(GreedyAgent));

    let turn = block_on(runtime.converse(&game, &"guide".into(), &alice.id, "hello"));
    assert!(turn.is_err(), "the missing leaderboard must fail the turn");
    assert_eq!(
        game.currency_balance(&alice.id, &"gold".into()).unwrap(),
        0,
        "a failed turn must not leave the GiveReward action applied"
    );
}

/// BUG 3: `ensure_account` is a read-then-write race; concurrent first logins
/// for the same id can return `AccountAlreadyExists`.
#[test]
fn probe_ensure_account_is_atomic() {
    let game = Arc::new(Game::new());
    game.create_region("asia", "Asia", None, None).unwrap();

    let barrier = Arc::new(std::sync::Barrier::new(16));
    let mut handles = Vec::new();
    for _ in 0..16 {
        let game = game.clone();
        let barrier = barrier.clone();
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            game.ensure_account("alice", "Alice", "asia")
        }));
    }
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let errors = results.iter().filter(|r| r.is_err()).count();
    assert_eq!(
        errors, 0,
        "concurrent ensure_account calls must all succeed"
    );
}

/// BUG 4: `Game::send_system_message` silently accepts a room/region channel
/// that does not exist, creating an orphaned chat log.
#[test]
fn probe_system_message_validates_channel() {
    let game = Game::new();
    game.create_region("asia", "Asia", None, None).unwrap();
    let result = game.send_system_message(ChatChannel::Room("room_missing".into()), "hello?");
    assert!(
        result.is_err(),
        "system messages to a non-existent room should be rejected"
    );
}

/// BUG 6: docs promise a task completed before its deadline can still be
/// claimed afterwards, but non-metric objectives are only stamped as completed
/// if something happens to query the task before expiry. `claim_task` skips
/// expired progress during its refresh, so a player who held the required item
/// for the whole window loses the task.
#[test]
fn probe_expired_task_with_met_item_objective() {
    let clock = Arc::new(ManualClock::new(1_000));
    let game = Game::with_clock(clock.clone());
    game.create_region("asia", "Asia", None, None).unwrap();
    let alice = game.create_account("alice", "Alice", "asia").unwrap();
    game.register_item(ItemDef::new("potion", "Potion"))
        .unwrap();
    game.register_task(
        TaskDef::new("timed", "Timed delivery")
            .with_objective(TaskObjective::item("potion", 1))
            .with_reward(Reward::new().currency("gold", 50))
            .with_time_limit_ms(1_000),
    )
    .unwrap();
    game.accept_task(&alice.id, &"timed".into()).unwrap();
    clock.advance(500);
    // Acquired comfortably inside the 1s window, but nothing queries the task
    // afterwards.
    game.add_item(&alice.id, &"potion".into(), 1).unwrap();
    clock.advance(1_500);
    // The potion was in the inventory for the entire (now expired) window.
    let claim = game.claim_task(&alice.id, &"timed".into());
    assert!(
        claim.is_ok(),
        "a task whose item objective was satisfied inside its window should remain claimable, got {claim:?}"
    );
}

/// BUG 5 (documentation vs behavior): Maintenance regions are documented as
/// "existing players may stay, but nobody new can enter", yet existing players
/// cannot create/join rooms either.
#[test]
fn probe_maintenance_region_existing_player_activity() {
    let game = Game::new();
    game.create_region("asia", "Asia", None, None).unwrap();
    let alice = game.create_account("alice", "Alice", "asia").unwrap();
    game.set_region_status(&"asia".into(), RegionStatus::Maintenance)
        .unwrap();

    let room = game.create_room(&alice.id, "Hangout", &"asia".into(), 8, BTreeMap::new());
    assert!(
        room.is_ok(),
        "existing players should be able to keep playing in a maintenance region"
    );
}
