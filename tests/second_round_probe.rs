//! Second-round inspection probes discovered while building the demo.
//!
//! All cases are `#[ignore]` so the first-round acceptance suite and the demo
//! smoke test stay green. Run them deliberately with:
//!
//! ```text
//! cargo test --test second_round_probe -- --ignored --test-threads=1
//! ```
//!
//! Current expectation: every case below fails, documenting a library gap or
//! a behavior/doc mismatch. After a library fix, remove `#[ignore]`.

use std::sync::Arc;

use taixu::prelude::*;

fn setup() -> (Game, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(1_000));
    let game = Game::with_clock(clock.clone());
    game.create_region("asia", "Asia", None, None).unwrap();
    game.create_region("eu", "Europe", None, None).unwrap();
    (game, clock)
}

fn account(game: &Game, id: &str) -> Account {
    game.create_account(id, id, "asia").unwrap()
}

// =========================================================================
// G1: repeatable task view keeps the previous cycle after cooldown
// =========================================================================

#[test]
#[ignore = "G1: repeatable task view exposes the previous cycle"]
fn probe_repeatable_task_available_after_cooldown_shows_fresh_progress() {
    let (game, clock) = setup();
    let alice = account(&game, "alice");

    game.register_task(
        TaskDef::new("daily", "Daily")
            .with_category("daily")
            .with_objective(TaskObjective::metric("kills", 1))
            .with_reward(Reward::new().currency("gold", 10))
            .with_repeat(TaskRepeat::cooldown(1_000)),
    )
    .unwrap();

    game.accept_task(&alice.id, &"daily".into()).unwrap();
    game.record_event(&alice.id, "kills", 1).unwrap();
    game.claim_task(&alice.id, &"daily".into()).unwrap();
    clock.advance(1_001);

    let view = game.task_view(&alice.id, &"daily".into()).unwrap();
    assert_eq!(view.status, TaskStatus::Available);
    assert_eq!(
        view.objectives[0].progress, 0,
        "an available repeatable task should present a fresh cycle, not the claimed cycle"
    );
    assert_eq!(view.completed_at_ms, None);
    assert_eq!(view.claimed_at_ms, None);
}

#[test]
#[ignore = "G1: cooldown over but currently ineligible should be Locked, not Claimed"]
fn probe_repeatable_task_locked_after_cooldown_reports_locked() {
    let (game, clock) = setup();
    let alice = account(&game, "alice");

    game.register_task(
        TaskDef::new("regional_daily", "Regional Daily")
            .with_category("daily")
            .with_objective(TaskObjective::metric("kills", 1))
            .with_reward(Reward::new().currency("gold", 10))
            .with_repeat(TaskRepeat::cooldown(1_000))
            .available_in([RegionId::from("asia")]),
    )
    .unwrap();

    game.accept_task(&alice.id, &"regional_daily".into())
        .unwrap();
    game.record_event(&alice.id, "kills", 1).unwrap();
    game.claim_task(&alice.id, &"regional_daily".into())
        .unwrap();
    clock.advance(1_001);
    game.set_account_region(&alice.id, "eu").unwrap();

    let view = game.task_view(&alice.id, &"regional_daily".into()).unwrap();
    assert_eq!(
        view.status,
        TaskStatus::Locked,
        "cooldown elapsed but the account is no longer in the task's region; \
         Claimed hides the actual lock reason"
    );
}

// =========================================================================
// G2: TaskView does not expose why a task is locked
// =========================================================================

#[test]
#[ignore = "G2: TaskView lacks requires/regions/window fields"]
fn probe_task_view_exposes_lock_reasons() {
    let (game, _) = setup();
    let alice = account(&game, "alice");

    game.register_task(
        TaskDef::new("locked", "Locked")
            .with_category("advanced")
            .with_objective(TaskObjective::metric("kills", 1))
            .with_reward(Reward::new().currency("gold", 10))
            .with_prerequisite("missing_prerequisite")
            .available_in([RegionId::from("asia")]),
    )
    .unwrap();

    let view = game.task_view(&alice.id, &"locked".into()).unwrap();
    let value = serde_json::to_value(&view).unwrap();
    for field in ["requires", "regions", "starts_at_ms", "ends_at_ms"] {
        assert!(
            value.get(field).is_some(),
            "TaskView should expose `{field}` so clients can explain Locked; got {value}"
        );
    }
}

// =========================================================================
// G4: trades_for is not chronological
// =========================================================================

#[test]
#[ignore = "G4: trades_for follows TradeId key order, not created_at_ms"]
fn probe_trades_for_returns_newest_first() {
    let (game, clock) = setup();
    let alice = account(&game, "alice");
    let bob = account(&game, "bob");

    for _ in 0..8 {
        game.create_trade(&alice.id, &bob.id).unwrap();
        clock.advance(1);
    }

    let trades = game.trades_for(&alice.id).unwrap();
    assert!(
        trades
            .windows(2)
            .all(|pair| pair[0].created_at_ms >= pair[1].created_at_ms),
        "trades_for should be newest-first; got {:?}",
        trades
            .iter()
            .map(|trade| (trade.id.to_string(), trade.created_at_ms))
            .collect::<Vec<_>>()
    );
}

// =========================================================================
// G6: Closed region still accepts new economic/chat activity
// =========================================================================

#[test]
#[ignore = "G6: docs say Closed = no new activity, but only room/entry checks exist"]
fn probe_closed_region_rejects_all_new_activity() {
    let (game, _) = setup();
    let alice = account(&game, "alice");

    game.register_item(ItemDef::new("potion", "Potion"))
        .unwrap();
    game.add_currency(&alice.id, &"gold".into(), 100).unwrap();
    let mut shop = Shop::new("shop", "Shop");
    shop.add_offer(ShopOffer::new(
        "offer",
        "Offer",
        Cost::new().currency("gold", 10),
        Reward::new().item("potion", 1),
    ))
    .unwrap();
    game.register_shop(shop).unwrap();

    game.set_region_status(&"asia".into(), RegionStatus::Closed)
        .unwrap();

    assert!(
        game.purchase(&alice.id, &"shop".into(), &"offer".into(), 1)
            .is_err(),
        "RegionStatus::Closed is documented as 'closed to all new activity', \
         but purchases still succeed"
    );
    assert!(
        game.send_message(&alice.id, ChatChannel::Region("asia".into()), "hello")
            .is_err(),
        "RegionStatus::Closed is documented as 'closed to all new activity', \
         but region chat still succeeds"
    );
}

// =========================================================================
// G7: GameState has no schema version
// =========================================================================

#[test]
#[ignore = "G7: snapshot persistence has no schema version/migration hook"]
fn probe_game_state_has_schema_version() {
    let game = Game::new();
    let value = serde_json::to_value(game.snapshot()).unwrap();
    assert!(
        value.get("version").is_some(),
        "GameState should carry a schema version for snapshot migrations; got keys {:?}",
        value
            .as_object()
            .map(|object| object.keys().cloned().collect::<Vec<_>>())
    );
}
