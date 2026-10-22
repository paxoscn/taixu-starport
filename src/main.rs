//! Taixu Starport — a small multiplayer web game hosted on the Taixu library.
//!
//! The library is synchronous and runtime-agnostic; Axum/Tokio live here in the
//! host. Sessions are host-issued bearer tokens (demo auth), while every
//! library call receives the trusted `AccountId` resolved from that session.

use std::env;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use taixu::clock::{system_now_ms, Clock, ManualClock, SystemClock};
use taixu::Game;

use taixu_demo::api;
use taixu_demo::seed::{build_npc_runtime, seed_world};
use taixu_demo::state::{demo_config, mark_all_offline, AppState};
use taixu_demo::store::FileStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let data_path = env::var("TAIXU_DATA").unwrap_or_else(|_| "data/state.json".into());
    let admin_token = env::var("TAIXU_ADMIN_TOKEN").unwrap_or_else(|_| "taixu-admin".into());
    let addr = env::var("TAIXU_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let manual = env::var("TAIXU_MANUAL_CLOCK")
        .map(|value| value == "1")
        .unwrap_or(false);

    let manual_clock = if manual {
        Some(Arc::new(ManualClock::new(system_now_ms())))
    } else {
        None
    };
    let clock: Arc<dyn Clock> = match &manual_clock {
        Some(clock) => clock.clone(),
        None => Arc::new(SystemClock),
    };

    let config = demo_config();
    let store = Arc::new(FileStore::new(&data_path));
    let game = match Game::load_from_with_clock(&*store, config.clone(), clock.clone())? {
        Some(game) => {
            println!("loaded snapshot from {}", store.path().display());
            game
        }
        None => {
            println!("no snapshot found; seeding a new world");
            let game = Game::with_config_and_clock(config, clock.clone());
            seed_world(&game)?;
            game.save_to(&*store)?;
            game
        }
    };
    mark_all_offline(&game);
    game.save_to(&*store)?;

    let game = Arc::new(game);
    let runtime = Arc::new(build_npc_runtime(&game));
    let state = Arc::new(AppState::new(
        game.clone(),
        store.clone(),
        runtime,
        admin_token.clone(),
        manual_clock,
    ));
    let app = api::router(state.clone());

    // Expire overdue trades even if nobody is online to tick them.
    let expiry_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        interval.tick().await;
        loop {
            interval.tick().await;
            let expired = expiry_state.game.expire_trades();
            if !expired.is_empty() {
                eprintln!("expired {} trade(s)", expired.len());
                if let Err(error) = expiry_state.persist() {
                    eprintln!("trade expiry save failed: {error:?}");
                }
            }
        }
    });

    // Presence sweep: a client that closes without logging out stops sending
    // heartbeats, so mark it offline after a grace period.
    let presence_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.tick().await;
        loop {
            interval.tick().await;
            let now = presence_state.now_ms();
            let stale: Vec<taixu::AccountId> = presence_state
                .game
                .read_state()
                .accounts
                .values()
                .filter(|account| {
                    account.online && now.saturating_sub(account.last_seen_ms) > 90_000
                })
                .map(|account| account.id.clone())
                .collect();
            if stale.is_empty() {
                continue;
            }
            for account in &stale {
                let _ = presence_state.game.logout(account);
            }
            if let Err(error) = presence_state.persist() {
                eprintln!("presence save failed: {error:?}");
            }
        }
    });

    // Periodic snapshot: captures lazy resource regeneration and online state.
    let save_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(error) = save_state.persist() {
                eprintln!("periodic save failed: {error:?}");
            }
        }
    });

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Taixu Starport: http://{addr}");
    println!("snapshot: {}", store.path().display());
    println!("admin token: {admin_token}");
    if manual {
        println!("manual clock enabled; advance via POST /api/admin/clock/advance");
    }
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    state.persist().ok();
    println!("snapshot saved on shutdown");
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    eprintln!("shutting down...");
}
