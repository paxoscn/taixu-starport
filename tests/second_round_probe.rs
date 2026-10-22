//! Second-round inspection probes discovered while building the demo.
//!
//! These probes were originally ignored while documenting the second-round
//! library gaps. The library fixes have landed (`511e5dd`), so they run as part
//! of the normal demo suite. G5 (retention / erase_account / purchase index) is
//! intentionally deferred to 0.2 and has no failing probe here.
//!
//! ```text
//! cargo test --test second_round_probe -- --test-threads=1
//! ```

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
// G1: repeatable task view starts a fresh cycle after cooldown
// =========================================================================

#[test]
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
// G2: TaskView exposes why a task is locked
// =========================================================================

#[test]
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
// G4: trades_for is newest-first
// =========================================================================

#[test]
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
// G6: Closed region rejects new economic/chat activity
// =========================================================================

#[test]
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
// G3: account-scoped channel summaries
// =========================================================================

#[test]
fn channels_for_returns_only_accessible_channels_newest_first() {
    let (game, clock) = setup();
    let alice = account(&game, "alice");
    let bob = account(&game, "bob");
    let carol = account(&game, "carol");

    let room = game
        .create_room(&alice.id, "Room", &"asia".into(), 8, Default::default())
        .unwrap();
    game.join_room(&bob.id, &room.id).unwrap();

    clock.advance(1);
    game.send_message(&alice.id, ChatChannel::Global, "global")
        .unwrap();
    clock.advance(1);
    game.send_message(&alice.id, ChatChannel::Region("asia".into()), "region")
        .unwrap();
    clock.advance(1);
    game.send_message(&alice.id, ChatChannel::Room(room.id.clone()), "room")
        .unwrap();
    clock.advance(1);
    game.send_message(
        &alice.id,
        ChatChannel::private(alice.id.clone(), bob.id.clone()),
        "dm a-b",
    )
    .unwrap();
    clock.advance(1);
    game.send_message(
        &bob.id,
        ChatChannel::private(bob.id.clone(), carol.id.clone()),
        "dm b-c",
    )
    .unwrap();
    clock.advance(1);
    game.send_message(&alice.id, ChatChannel::Global, "global latest")
        .unwrap();

    let channels = game.channels_for(&alice.id).unwrap();
    let has = |target: &ChatChannel| channels.iter().any(|summary| &summary.channel == target);
    assert!(has(&ChatChannel::Global));
    assert!(has(&ChatChannel::Region("asia".into())));
    assert!(has(&ChatChannel::Room(room.id.clone())));
    assert!(has(&ChatChannel::private(alice.id.clone(), bob.id.clone())));
    assert!(
        !has(&ChatChannel::private(bob.id.clone(), carol.id.clone())),
        "Alice must not see Bob/Carol's DM"
    );
    assert_eq!(
        channels[0].channel,
        ChatChannel::Global,
        "newest activity first"
    );
    assert_eq!(channels[0].message_count, 2);
}

// =========================================================================
// G5 is deferred by design (0.2): retention / erase_account / purchase index.
// =========================================================================

// =========================================================================
// G7: GameState has a schema version
// =========================================================================

#[test]
fn probe_game_state_has_schema_version() {
    let game = Game::new();
    assert_eq!(game.state_version(), GAME_STATE_VERSION);
    let value = serde_json::to_value(game.snapshot()).unwrap();
    assert_eq!(
        value.get("version").and_then(|version| version.as_u64()),
        Some(u64::from(GAME_STATE_VERSION))
    );
}

// =========================================================================
// G8: with_state is a safe short-query alternative to a read guard
// =========================================================================

#[test]
fn with_state_runs_a_short_read_query() {
    let (game, _) = setup();
    let alice = account(&game, "alice");
    let (count, has_alice) =
        game.with_state(|state| (state.accounts.len(), state.accounts.contains_key(&alice.id)));
    assert_eq!(count, 1);
    assert!(has_alice);
}

// =========================================================================
// G9: avatar frame/badge and equipped skins stay in sync
// =========================================================================

#[test]
fn avatar_and_equipped_skins_stay_in_sync() {
    let (game, _) = setup();
    let alice = account(&game, "alice");

    game.register_skin(SkinDef::new("frame", "Frame", SkinCategory::Frame))
        .unwrap();
    game.grant_skin(&alice.id, &"frame".into()).unwrap();
    game.equip_skin(&alice.id, &"frame".into()).unwrap();

    let account = game.account(&alice.id).unwrap();
    assert_eq!(account.avatar.frame, Some(SkinId::from("frame")));
    assert_eq!(
        game.equipped_skins(&alice.id)
            .unwrap()
            .get(&SkinCategory::Frame),
        Some(&SkinId::from("frame"))
    );

    // update_avatar is authoritative: clearing the frame unequips it.
    let mut avatar = account.avatar;
    avatar.frame = None;
    game.update_avatar(&alice.id, avatar).unwrap();
    assert_eq!(game.account(&alice.id).unwrap().avatar.frame, None);
    assert!(!game
        .equipped_skins(&alice.id)
        .unwrap()
        .contains_key(&SkinCategory::Frame));

    // Setting it via update_avatar equips it again.
    let mut avatar = game.account(&alice.id).unwrap().avatar;
    avatar.frame = Some(SkinId::from("frame"));
    game.update_avatar(&alice.id, avatar).unwrap();
    assert_eq!(
        game.equipped_skins(&alice.id)
            .unwrap()
            .get(&SkinCategory::Frame),
        Some(&SkinId::from("frame"))
    );

    // unequip_skin clears the avatar slot.
    game.unequip_skin(&alice.id, &SkinCategory::Frame).unwrap();
    assert_eq!(game.account(&alice.id).unwrap().avatar.frame, None);
}
