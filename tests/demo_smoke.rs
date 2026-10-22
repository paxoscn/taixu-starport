//! Host-level smoke test: seeded world, gameplay composites, NPC turn,
//! deterministic clock, and MemoryStore snapshot round-trip.

use std::sync::Arc;

use futures::executor::block_on;
use taixu::prelude::*;
use taixu::store::MemoryStore;

use taixu_demo::gameplay;
use taixu_demo::seed::{build_npc_runtime, seed_world, REGION_AURORA, REGION_NEBULA};
use taixu_demo::state::demo_config;

fn starter() -> Reward {
    Reward::new()
        .currency("gold", 120)
        .currency("gems", 1)
        .resource("energy", 60)
        .resource("focus", 10)
        .item("potion", 2)
        .item("ore", 3)
        .skin("char_ranger")
}

#[test]
fn seeded_world_supports_full_gameplay_and_snapshot() {
    let clock = Arc::new(ManualClock::new(1_700_000_000_000));
    let config = demo_config();
    let game = Game::with_config_and_clock(config.clone(), clock.clone());
    seed_world(&game).unwrap();

    let alice = game
        .ensure_account("player_alice", "Alice", REGION_AURORA)
        .unwrap();
    game.grant_reward(&alice.id, &starter()).unwrap();
    assert_eq!(game.item_count(&alice.id, &"potion".into()).unwrap(), 2);
    assert_eq!(
        game.currency_balance(&alice.id, &"gold".into()).unwrap(),
        120
    );
    assert!(game.owns_skin(&alice.id, &"char_ranger".into()).unwrap());

    // Tutorial task + expedition event.
    game.accept_task(&alice.id, &"tut_expedition".into())
        .unwrap();
    let gather = gameplay::explore(&game, &alice.id, "gather").unwrap();
    assert!(!gather.reward.is_empty());
    assert_eq!(
        game.resource_balance(&alice.id, &"energy".into()).unwrap(),
        55
    );

    let expedition = gameplay::explore(&game, &alice.id, "expedition").unwrap();
    assert!(expedition
        .outcome
        .tasks
        .iter()
        .any(|task| task.as_str() == "tut_expedition"));
    assert!(game
        .leaderboard_score(&"speed".into(), &alice.id)
        .unwrap()
        .is_some());
    assert_eq!(
        game.task_view(&alice.id, &"tut_expedition".into())
            .unwrap()
            .status,
        TaskStatus::Completed
    );

    // Shop purchase (currency + resource cost, stock/limit accounting).
    let receipt = game
        .purchase(&alice.id, &"general".into(), &"supply_pack".into(), 1)
        .unwrap();
    assert_eq!(receipt.quantity, 1);
    assert_eq!(
        game.purchased_quantity(&alice.id, &"general".into(), &"supply_pack".into()),
        1
    );

    // Host composites: craft and sell, with the host action lock in the API.
    game.add_item(&alice.id, &"ore".into(), 3).unwrap();
    let crafted = gameplay::craft(&game, &alice.id, "rift_shard").unwrap();
    assert!(crafted
        .reward
        .items
        .iter()
        .any(|grant| grant.item.as_str() == "rift_shard"));
    let sold = gameplay::sell(&game, &alice.id, "potion", 1).unwrap();
    assert!(sold.earned > 0);

    // NPC runtime: Say + GiveReward, then SetScore.
    let runtime = build_npc_runtime(&game);
    let supply =
        block_on(runtime.converse(&game, &"guide".into(), &alice.id, "请给我补给")).unwrap();
    assert_eq!(supply.applied_actions, 1);
    assert!(!supply.npc_messages.is_empty());

    let fortune = block_on(runtime.converse(&game, &"guide".into(), &alice.id, "占卜")).unwrap();
    assert_eq!(fortune.applied_actions, 1);
    assert!(game
        .leaderboard_score(&"fortune".into(), &alice.id)
        .unwrap()
        .is_some());

    // Announcement read/unread and collection progress.
    let unread_before = game.unread_announcements(&alice.id).unwrap();
    assert!(!unread_before.is_empty());
    game.mark_announcement_read(&alice.id, &unread_before[0].id)
        .unwrap();
    assert_eq!(
        game.unread_announcements(&alice.id).unwrap().len(),
        unread_before.len() - 1
    );
    let collection = game
        .collection_progress(&alice.id, &"starport_album".into())
        .unwrap();
    assert!(collection.collected > 0);

    // Region migration is blocked into a closed region, allowed into an open one.
    assert!(game.set_account_region(&alice.id, "drift").is_err());
    let moved = game.set_account_region(&alice.id, REGION_NEBULA).unwrap();
    assert_eq!(moved.region, REGION_NEBULA.into());

    // Snapshot round-trip through the library's MemoryStore.
    let store = MemoryStore::new();
    game.save_to(&store).unwrap();
    let restored = Game::load_from_with_clock(&store, config, clock.clone())
        .unwrap()
        .expect("snapshot exists");
    assert_eq!(restored.account(&alice.id).unwrap().display_name, "Alice");
    assert_eq!(
        restored.account(&alice.id).unwrap().region,
        REGION_NEBULA.into()
    );

    // Manual clock drives lazy resource regeneration deterministically.
    restored
        .set_resource(&alice.id, &"energy".into(), 0)
        .unwrap();
    clock.advance(3_600_000);
    assert_eq!(
        restored
            .resource_balance(&alice.id, &"energy".into())
            .unwrap(),
        20
    );
}
