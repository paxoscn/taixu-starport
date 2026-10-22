//! HTTP surface: one JSON state document per poll, plus small command routes.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use axum::{
    extract::{FromRequestParts, Query, State},
    http::{header, HeaderMap},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use taixu::npc::NpcError;
use taixu::prelude::*;
use taixu::GameError;

use crate::error::{ApiError, ApiResult};
use crate::gameplay;
use crate::seed::SYSTEM_ACCOUNT;
use crate::state::{AppState, AuthUser};

const INDEX_HTML: &str = include_str!("../web/index.html");
const APP_JS: &str = include_str!("../web/app.js");
const STYLE_CSS: &str = include_str!("../web/style.css");

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/style.css", get(style_css))
        .route("/api/health", get(health))
        .route("/api/world", get(world))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/heartbeat", post(heartbeat))
        .route("/api/state", get(get_state))
        .route("/api/action/explore", post(explore))
        .route("/api/inventory/craft", post(craft))
        .route("/api/inventory/sell", post(sell))
        .route("/api/inventory/upgrade", post(upgrade))
        .route("/api/travel", post(travel))
        .route("/api/rooms/create", post(room_create))
        .route("/api/rooms/join", post(room_join))
        .route("/api/rooms/leave", post(room_leave))
        .route("/api/rooms/close", post(room_close))
        .route("/api/rooms/kick", post(room_kick))
        .route("/api/rooms/metadata", post(room_metadata))
        .route("/api/rooms/metadata/remove", post(room_metadata_remove))
        .route("/api/chat/send", post(chat_send))
        .route("/api/chat/invite-trade", post(chat_invite_trade))
        .route("/api/shop/buy", post(shop_buy))
        .route("/api/trades/create", post(trade_create))
        .route("/api/trades/side", post(trade_side))
        .route("/api/trades/confirm", post(trade_confirm))
        .route("/api/trades/cancel", post(trade_cancel))
        .route("/api/friends/request", post(friend_request))
        .route("/api/friends/accept", post(friend_accept))
        .route("/api/friends/decline", post(friend_decline))
        .route("/api/friends/cancel", post(friend_cancel))
        .route("/api/friends/remove", post(friend_remove))
        .route("/api/friends/block", post(friend_block))
        .route("/api/friends/unblock", post(friend_unblock))
        .route("/api/profile", post(update_profile))
        .route("/api/profile/avatar", post(update_avatar))
        .route("/api/skins/equip", post(equip_skin))
        .route("/api/skins/unequip", post(unequip_skin))
        .route("/api/announcements/read", post(announcement_read))
        .route("/api/achievements/claim", post(achievement_claim))
        .route("/api/tasks/accept", post(task_accept))
        .route("/api/tasks/abandon", post(task_abandon))
        .route("/api/tasks/claim", post(task_claim))
        .route("/api/collections/claim", post(collection_claim))
        .route("/api/npc/say", post(npc_say))
        .route("/api/npc/prompt", post(npc_prompt))
        .route("/api/admin/overview", get(admin_overview))
        .route("/api/admin/accounts", get(admin_accounts))
        .route("/api/admin/tasks", get(admin_tasks))
        .route("/api/admin/announcements", get(admin_announcements))
        .route("/api/admin/npcs", get(admin_npcs))
        .route("/api/admin/snapshot", get(admin_snapshot))
        .route("/api/admin/announcement", post(admin_announcement))
        .route(
            "/api/admin/announcement/enabled",
            post(admin_announcement_enabled),
        )
        .route(
            "/api/admin/announcement/remove",
            post(admin_announcement_remove),
        )
        .route("/api/admin/region/status", post(admin_region_status))
        .route(
            "/api/admin/leaderboard/reset",
            post(admin_leaderboard_reset),
        )
        .route("/api/admin/chat/delete", post(admin_chat_delete))
        .route("/api/admin/chat/clear", post(admin_chat_clear))
        .route("/api/admin/grant", post(admin_grant))
        .route("/api/admin/capacity", post(admin_capacity))
        .route("/api/admin/npc/enabled", post(admin_npc_enabled))
        .route("/api/admin/npc/remove", post(admin_npc_remove))
        .route("/api/admin/offer/remove", post(admin_offer_remove))
        .route("/api/admin/clock/advance", post(admin_clock_advance))
        .route("/api/admin/trade/expire", post(admin_trade_expire))
        .route("/api/admin/save", post(admin_save))
        .with_state(state)
}

// =========================================================================
// Static + public
// =========================================================================

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn app_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        APP_JS,
    )
}

async fn style_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        STYLE_CSS,
    )
}

async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "now_ms": state.now_ms(),
        "manual_clock": state.manual_clock.is_some(),
    }))
}

async fn world(State(state): State<Arc<AppState>>) -> ApiResult<Json<Value>> {
    let game = &state.game;
    let regions: Vec<Value> = game
        .regions()
        .into_iter()
        .map(|region| {
            json!({
                "id": region.id,
                "name": region.name,
                "endpoint": region.endpoint,
                "capacity": region.capacity,
                "status": region.status,
                "players": region.player_count(),
                "created_at_ms": region.created_at_ms,
            })
        })
        .collect();
    let npcs: Vec<Value> = game
        .npcs()
        .into_iter()
        .map(|npc| {
            json!({
                "id": npc.id,
                "name": npc.name,
                "channel": npc.channel().to_string(),
            })
        })
        .collect();
    Ok(Json(json!({
        "regions": regions,
        "npcs": npcs,
        "now_ms": state.now_ms(),
        "manual_clock": state.manual_clock.is_some(),
        "accounts": game.account_count(),
    })))
}

// =========================================================================
// Auth
// =========================================================================

#[derive(Deserialize)]
struct LoginReq {
    name: String,
    #[serde(default)]
    region: String,
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginReq>,
) -> ApiResult<Json<Value>> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("名字不能为空"));
    }
    if name.eq_ignore_ascii_case("system") {
        return Err(ApiError::forbidden("该名字为系统保留"));
    }
    if name.chars().count() > state.game.config().max_name_len {
        return Err(ApiError::bad_request(format!(
            "名字最多 {} 个字符",
            state.game.config().max_name_len
        )));
    }
    let region = req.region.trim();
    if region.is_empty() {
        return Err(ApiError::bad_request("请选择一个区域"));
    }

    let id: AccountId = format!("player_{name}").into();
    let _guard = state.action_lock.lock();
    let existing = state.game.account(&id).ok();
    let account = state.game.ensure_account(id.clone(), name, region)?;
    if existing.is_none() {
        // Starter kit: all five reward kinds are exercised in one atomic grant.
        state.game.grant_reward(
            &account.id,
            &Reward::new()
                .currency("gold", 120)
                .currency("gems", 1)
                .resource("energy", 60)
                .resource("focus", 10)
                .item("potion", 2)
                .item("ore", 3)
                .skin("char_ranger"),
        )?;
        state
            .game
            .set_account_metadata(&account.id, "bio", "初到星港的旅行者")?;
        let _ = state.game.send_system_message(
            ChatChannel::Global,
            &format!("✦ {} 登上了太虚星港", account.display_name),
        );
        let _ = gameplay::sync_level(&state.game, &account.id)?;
    }
    let account = state.game.login(&account.id)?;
    let token = state.create_session(&account.id);
    drop(_guard);
    state.persist()?;
    Ok(Json(json!({ "token": token, "account": account })))
}

async fn logout(State(state): State<Arc<AppState>>, user: AuthUser) -> ApiResult<Json<Value>> {
    let _ = state.game.logout(&user.account);
    state.remove_session(&user.token);
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

async fn heartbeat(State(state): State<Arc<AppState>>, user: AuthUser) -> ApiResult<Json<Value>> {
    state.game.heartbeat(&user.account)?;
    state.touch_session(&user.token);
    Ok(Json(json!({ "ok": true, "now_ms": state.now_ms() })))
}

// =========================================================================
// State aggregation
// =========================================================================

#[derive(Deserialize)]
struct StateQuery {
    #[serde(default)]
    channel: String,
}

async fn get_state(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    headers: HeaderMap,
    Query(query): Query<StateQuery>,
) -> ApiResult<Json<Value>> {
    let channel = if query.channel.trim().is_empty() {
        "global"
    } else {
        query.channel.as_str()
    };
    build_state(&state, &user.account, &headers, channel)
}

fn name_of(names: &BTreeMap<String, Value>, id: &AccountId) -> String {
    names
        .get(id.as_str())
        .and_then(|entry| entry.get("display_name"))
        .and_then(Value::as_str)
        .unwrap_or(id.as_str())
        .to_string()
}

fn item_name(items: &BTreeMap<String, ItemDef>, id: &ItemId) -> String {
    items
        .get(id.as_str())
        .map(|def| def.name.clone())
        .unwrap_or_else(|| id.to_string())
}

fn skin_name(skins: &BTreeMap<String, SkinDef>, id: &SkinId) -> String {
    skins
        .get(id.as_str())
        .map(|def| def.name.clone())
        .unwrap_or_else(|| id.to_string())
}

fn achievement_name(achievements: &BTreeMap<String, AchievementDef>, id: &AchievementId) -> String {
    achievements
        .get(id.as_str())
        .map(|def| def.name.clone())
        .unwrap_or_else(|| id.to_string())
}

fn names_map(game: &Game) -> BTreeMap<String, Value> {
    let snapshot = game.read_state();
    snapshot
        .accounts
        .values()
        .map(|account| {
            (
                account.id.to_string(),
                json!({
                    "id": account.id,
                    "display_name": account.display_name,
                    "level": account.level,
                    "online": account.online,
                    "region": account.region,
                    "avatar": account.avatar,
                    "is_system": account.id.as_str() == SYSTEM_ACCOUNT,
                }),
            )
        })
        .collect()
}

fn room_json(room: &Room, names: &BTreeMap<String, Value>, me: &AccountId) -> Value {
    let players: Vec<Value> = room
        .players
        .iter()
        .map(|id| {
            json!({
                "id": id,
                "name": name_of(names, id),
                "online": names.get(id.as_str()).and_then(|e| e.get("online")).and_then(Value::as_bool).unwrap_or(false),
                "is_owner": *id == room.owner,
            })
        })
        .collect();
    json!({
        "id": room.id,
        "name": room.name,
        "region": room.region,
        "owner": room.owner,
        "owner_name": name_of(names, &room.owner),
        "capacity": room.capacity,
        "player_count": room.players.len(),
        "players": players,
        "metadata": room.metadata,
        "created_at_ms": room.created_at_ms,
        "updated_at_ms": room.updated_at_ms,
        "joined": room.contains(me),
        "is_owner": room.owner == *me,
    })
}

fn messages_json(
    game: &Game,
    names: &BTreeMap<String, Value>,
    messages: &[ChatMessage],
) -> Vec<Value> {
    messages
        .iter()
        .map(|message| {
            let author = match &message.author {
                ChatAuthor::Account { account } => json!({
                    "kind": "account",
                    "id": account,
                    "name": name_of(names, account),
                }),
                ChatAuthor::Npc { npc } => {
                    let name = game
                        .npc(npc)
                        .map(|def| def.name)
                        .unwrap_or_else(|_| npc.to_string());
                    json!({ "kind": "npc", "id": npc, "name": name })
                }
                ChatAuthor::System => json!({ "kind": "system", "name": "星港系统" }),
            };
            json!({
                "id": message.id,
                "channel": message.channel.to_string(),
                "author": author,
                "body": message.body,
                "sent_at_ms": message.sent_at_ms,
                "metadata": message.metadata,
            })
        })
        .collect()
}

fn channel_label(
    game: &Game,
    names: &BTreeMap<String, Value>,
    me: &AccountId,
    channel: &ChatChannel,
) -> String {
    match channel {
        ChatChannel::Global => "全局频道".into(),
        ChatChannel::Region(region) => format!(
            "区域 · {}",
            game.region(region)
                .map(|region| region.name)
                .unwrap_or_else(|_| region.to_string())
        ),
        ChatChannel::Room(room) => format!(
            "房间 · {}",
            game.room(room)
                .map(|room| room.name)
                .unwrap_or_else(|_| room.to_string())
        ),
        ChatChannel::Private { a, b } => {
            let other = if a == me { b } else { a };
            format!("私聊 · {}", name_of(names, other))
        }
    }
}

fn trade_json(
    trade: &Trade,
    me: &AccountId,
    names: &BTreeMap<String, Value>,
    items: &BTreeMap<String, ItemDef>,
    now_ms: i64,
) -> Value {
    let mine = if trade.initiator == *me {
        &trade.initiator_side
    } else {
        &trade.partner_side
    };
    let theirs = if trade.initiator == *me {
        &trade.partner_side
    } else {
        &trade.initiator_side
    };
    let side = |side: &TradeSide, other_name: Option<&str>| {
        json!({
            "items": side.items.iter().map(|(id, quantity)| json!({
                "id": id,
                "name": item_name(items, id),
                "quantity": quantity,
            })).collect::<Vec<_>>(),
            "currencies": side.currencies,
            "empty": side.is_empty(),
            "partner_name": other_name,
        })
    };
    let partner = trade.other(me);
    json!({
        "id": trade.id,
        "initiator": trade.initiator,
        "partner": trade.partner,
        "partner_name": partner.map(|id| name_of(names, id)).unwrap_or_default(),
        "my_side": side(mine, partner.map(|id| names.get(id.as_str()).and_then(|e| e.get("display_name")).and_then(Value::as_str).unwrap_or(id.as_str()))),
        "their_side": side(theirs, None),
        "my_confirmed": trade.confirmed(me),
        "their_confirmed": partner.is_some_and(|id| trade.confirmed(id)),
        "state": trade.state,
        "created_at_ms": trade.created_at_ms,
        "expires_at_ms": trade.expires_at_ms,
        "expired": trade.is_expired(now_ms),
        "done": trade.state != TradeState::Open,
    })
}

fn parse_enum<T: DeserializeOwned>(value: &str, what: &str) -> ApiResult<T> {
    serde_json::from_value(Value::String(value.to_owned()))
        .map_err(|_| ApiError::bad_request(format!("无效的{what}: {value}")))
}

fn build_state(
    state: &AppState,
    me: &AccountId,
    headers: &HeaderMap,
    channel_raw: &str,
) -> ApiResult<Json<Value>> {
    let game = &state.game;
    let now = game.now_ms();
    let account = game.account(me)?;
    let wallet = game.wallet(me)?;
    let inventory = game.inventory(me)?;
    let names = names_map(game);

    let items_vec = game.items();
    let item_map: BTreeMap<String, ItemDef> = items_vec
        .iter()
        .cloned()
        .map(|def| (def.id.to_string(), def))
        .collect();
    let skins_vec = game.skins();
    let skin_map: BTreeMap<String, SkinDef> = skins_vec
        .iter()
        .cloned()
        .map(|def| (def.id.to_string(), def))
        .collect();
    let achievements_vec = game.achievements_for(me)?;
    let achievement_map: BTreeMap<String, AchievementDef> = game
        .achievements()
        .into_iter()
        .map(|def| (def.id.to_string(), def))
        .collect();

    // ---- me / level curve ------------------------------------------------
    let (xp_floor, xp_next) = gameplay::xp_bounds(account.level);
    let xp_pct = if xp_next > xp_floor {
        ((account.xp.saturating_sub(xp_floor)) * 100 / (xp_next - xp_floor)).min(100)
    } else {
        100
    };
    let me_json = json!({
        "id": account.id,
        "display_name": account.display_name,
        "region": account.region,
        "level": account.level,
        "xp": account.xp,
        "xp_floor": xp_floor,
        "xp_next": xp_next,
        "xp_pct": xp_pct,
        "level_from_xp": gameplay::level_from_xp(account.xp),
        "online": account.online,
        "created_at_ms": account.created_at_ms,
        "last_seen_ms": account.last_seen_ms,
        "avatar": account.avatar,
        "metadata": account.metadata,
    });

    // ---- regions ---------------------------------------------------------
    let regions: Vec<Value> = game
        .regions()
        .into_iter()
        .map(|region| {
            let players = game.region_player_count(&region.id).unwrap_or(0);
            json!({
                "id": region.id,
                "name": region.name,
                "endpoint": region.endpoint,
                "capacity": region.capacity,
                "status": region.status,
                "players": players,
                "created_at_ms": region.created_at_ms,
                "metadata": region.metadata,
                "is_mine": region.id == account.region,
            })
        })
        .collect();

    // ---- rooms -----------------------------------------------------------
    let current_room_id = game.room_id_of(me)?;
    let current_room = match &current_room_id {
        Some(id) => Some(room_json(&game.room(id)?, &names, me)),
        None => None,
    };
    let rooms: Vec<Value> = game
        .rooms(Some(&account.region))
        .iter()
        .map(|room| room_json(room, &names, me))
        .collect();

    // ---- friends ---------------------------------------------------------
    let friends: Vec<Value> = game
        .friends(me)?
        .iter()
        .map(|id| {
            let summary = names
                .get(id.as_str())
                .cloned()
                .unwrap_or_else(|| json!({ "id": id, "display_name": id.to_string() }));
            summary
        })
        .collect();
    let incoming: Vec<Value> = game
        .friend_requests(me)?
        .iter()
        .map(|request| {
            json!({
                "from": request.from,
                "from_name": name_of(&names, &request.from),
                "message": request.message,
                "created_at_ms": request.created_at_ms,
            })
        })
        .collect();
    let outgoing: Vec<Value> = game
        .outgoing_friend_requests(me)?
        .iter()
        .map(|request| {
            json!({
                "to": request.to,
                "to_name": name_of(&names, &request.to),
                "message": request.message,
                "created_at_ms": request.created_at_ms,
            })
        })
        .collect();
    let blocked: Vec<Value> = game
        .blocked_accounts(me)?
        .iter()
        .map(|id| json!({ "id": id, "name": name_of(&names, id) }))
        .collect();

    // ---- trades ----------------------------------------------------------
    // `TradeBook` is keyed by TradeId, so expose newest-first explicitly.
    let mut trade_list = game.trades_for(me)?;
    trade_list.sort_by(|a, b| {
        b.created_at_ms
            .cmp(&a.created_at_ms)
            .then_with(|| b.id.cmp(&a.id))
    });
    let trades: Vec<Value> = trade_list
        .iter()
        .map(|trade| trade_json(trade, me, &names, &item_map, now))
        .collect();

    // ---- tasks -----------------------------------------------------------
    let tasks = game.task_views(me)?;

    // ---- collections -----------------------------------------------------
    let owned_skins = game.owned_skins(me)?;
    let completed_achievements: BTreeSet<String> = achievements_vec
        .iter()
        .filter(|achievement| achievement.completed)
        .map(|achievement| achievement.id.to_string())
        .collect();
    let collections: Vec<Value> = game
        .collections()
        .into_iter()
        .map(|def| {
            let progress = game.collection_progress(me, &def.id)?;
            let entries: Vec<Value> = def
                .entries
                .iter()
                .map(|entry| match entry {
                    Collectible::Item(id) => json!({
                        "kind": "item",
                        "id": id,
                        "name": item_name(&item_map, id),
                        "owned": inventory.count(id) > 0,
                    }),
                    Collectible::Skin(id) => json!({
                        "kind": "skin",
                        "id": id,
                        "name": skin_name(&skin_map, id),
                        "owned": owned_skins.owns(id),
                    }),
                    Collectible::Achievement(id) => json!({
                        "kind": "achievement",
                        "id": id,
                        "name": achievement_name(&achievement_map, id),
                        "owned": completed_achievements.contains(id.as_str()),
                    }),
                })
                .collect();
            Ok::<Value, GameError>(json!({
                "definition": def,
                "progress": progress,
                "entries": entries,
            }))
        })
        .collect::<taixu::Result<Vec<_>>>()?;

    // ---- leaderboards ----------------------------------------------------
    let leaderboards: Vec<Value> = game
        .leaderboards()
        .into_iter()
        .map(|board| {
            let top: Vec<Value> = game
                .leaderboard_top(&board.id, 10)?
                .iter()
                .map(|entry| {
                    json!({
                        "rank": entry.rank,
                        "account": entry.account,
                        "name": name_of(&names, &entry.account),
                        "score": entry.score,
                    })
                })
                .collect();
            let my_score = game.leaderboard_score(&board.id, me)?;
            let my_rank = game.leaderboard_rank(&board.id, me)?;
            Ok::<Value, GameError>(json!({
                "id": board.id,
                "name": board.name,
                "description": board.description,
                "order": board.order,
                "season": board.season,
                "max_entries": board.max_entries,
                "top": top,
                "my_score": my_score,
                "my_rank": my_rank,
            }))
        })
        .collect::<taixu::Result<Vec<_>>>()?;

    // ---- shops -----------------------------------------------------------
    let shops: Vec<Value> = game
        .shops()
        .into_iter()
        .map(|shop| {
            let open = shop.is_open_at(now, &account.region);
            let offers: Vec<Value> = shop
                .offers
                .values()
                .map(|offer| {
                    let purchased = game.purchased_quantity(me, &shop.id, &offer.id);
                    let region_ok = offer
                        .regions
                        .as_ref()
                        .is_none_or(|regions| regions.contains(&account.region));
                    let window_ok = offer.available_at(now, &account.region);
                    json!({
                        "id": offer.id,
                        "name": offer.name,
                        "description": offer.description,
                        "cost": offer.cost,
                        "reward": offer.reward,
                        "starts_at_ms": offer.starts_at_ms,
                        "ends_at_ms": offer.ends_at_ms,
                        "regions": offer.regions,
                        "per_account_limit": offer.per_account_limit,
                        "purchased": purchased,
                        "stock": offer.stock,
                        "sold": offer.sold,
                        "remaining_stock": offer.remaining_stock(),
                        "tags": offer.tags,
                        "available": open && window_ok,
                        "region_ok": region_ok,
                        "window_ok": window_ok,
                    })
                })
                .collect();
            json!({
                "id": shop.id,
                "name": shop.name,
                "description": shop.description,
                "regions": shop.regions,
                "opens_at_ms": shop.opens_at_ms,
                "closes_at_ms": shop.closes_at_ms,
                "open": open,
                "offers": offers,
            })
        })
        .collect();

    // ---- announcements ---------------------------------------------------
    let announcements: Vec<Value> = game
        .active_announcements(me)?
        .into_iter()
        .map(|announcement| {
            let read = game.is_announcement_read(me, &announcement.id);
            json!({
                "id": announcement.id,
                "title": announcement.title,
                "body": announcement.body,
                "kind": announcement.kind,
                "priority": announcement.priority,
                "starts_at_ms": announcement.starts_at_ms,
                "ends_at_ms": announcement.ends_at_ms,
                "regions": announcement.regions,
                "enabled": announcement.enabled,
                "created_at_ms": announcement.created_at_ms,
                "read": read,
            })
        })
        .collect();
    let unread_announcements = game.unread_announcements(me)?.len();

    // ---- cosmetics -------------------------------------------------------
    let owned_skins_json = json!({
        "catalog": skins_vec,
        "owned": owned_skins.owned().iter().map(ToString::to_string).collect::<Vec<_>>(),
        "equipped": owned_skins.equipped(),
    });

    // ---- NPCs accessible from the player's current location --------------
    let npcs: Vec<Value> = game
        .npcs()
        .into_iter()
        .filter(|npc| {
            if let Some(room) = &npc.room {
                current_room_id.as_ref() == Some(room)
            } else if let Some(region) = &npc.region {
                *region == account.region
            } else {
                true
            }
        })
        .map(|npc| {
            json!({
                "id": npc.id,
                "name": npc.name,
                "persona": npc.persona,
                "channel": npc.channel().to_string(),
                "greeting": npc.greeting,
                "metadata": npc.metadata,
                "enabled": npc.enabled,
            })
        })
        .collect();

    // ---- chat ------------------------------------------------------------
    let (channel, channel_value) = match gameplay::user_channel(game, me, channel_raw) {
        Ok(channel) => {
            let value = gameplay::channel_value(&channel, me);
            (channel, value)
        }
        Err(_) => (ChatChannel::Global, "global".to_string()),
    };
    let (messages, chat_error) = match game.recent_messages(me, channel.clone(), 150) {
        Ok(messages) => (messages_json(game, &names, &messages), Value::Null),
        Err(error) => (Vec::new(), json!(error.to_string())),
    };
    let chat = json!({
        "channel": channel_value,
        "label": channel_label(game, &names, me, &channel),
        "messages": messages,
        "error": chat_error,
    });

    // ---- inventory -------------------------------------------------------
    let slots: Vec<Value> = inventory
        .slots()
        .iter()
        .enumerate()
        .map(|(index, slot)| match slot {
            Some(stack) => json!({
                "index": index,
                "item": stack.item(),
                "quantity": stack.quantity(),
            }),
            None => json!({ "index": index, "item": null, "quantity": 0 }),
        })
        .collect();

    let snapshot = game.read_state();
    let online = snapshot
        .accounts
        .values()
        .filter(|account| account.online)
        .count();
    let is_admin = state.is_admin(headers);
    let metric_labels: BTreeMap<String, String> = [
        "expeditions",
        "gathers",
        "boss",
        "secret_found",
        "trades",
        "messages",
        "purchases",
        "crafts",
        "sales",
        "upgrades",
    ]
    .iter()
    .map(|metric| {
        (
            (*metric).to_string(),
            gameplay::metric_label(metric).to_string(),
        )
    })
    .collect();

    Ok(Json(json!({
        "server": {
            "now_ms": now,
            "started_at_ms": state.started_at_ms,
            "manual_clock": state.manual_clock.is_some(),
            "accounts": snapshot.accounts.len(),
            "online": online,
            "sessions": state.session_count(),
            "trade_expiry_secs": game.config().trade_expiry_secs,
            "store": state.store.path().display().to_string(),
        },
        "me": me_json,
        "wallet": {
            "currencies": wallet.currencies,
            "resources": wallet.resources,
            "resource_defs": game.resources(),
        },
        "inventory": {
            "capacity": inventory.capacity(),
            "used_slots": inventory.used_slots(),
            "free_slots": inventory.free_slots(),
            "slots": slots,
        },
        "regions": regions,
        "room": current_room,
        "rooms": rooms,
        "friends": friends,
        "friend_requests": incoming,
        "outgoing_requests": outgoing,
        "blocked": blocked,
        "trades": trades,
        "tasks": tasks,
        "task_hints": gameplay::task_hints(),
        "achievements": achievements_vec,
        "collections": collections,
        "leaderboards": leaderboards,
        "shops": shops,
        "announcements": announcements,
        "unread_announcements": unread_announcements,
        "skins": owned_skins_json,
        "npcs": npcs,
        "chat": chat,
        "catalogs": {
            "items": items_vec,
            "skins": skins_vec,
            "resources": game.resources(),
            "currencies": gameplay::currency_meta(),
            "metric_labels": metric_labels,
        },
        "names": names,
        "admin": is_admin,
    })))
}

// =========================================================================
// Player actions
// =========================================================================

#[derive(Deserialize)]
struct ExploreReq {
    kind: String,
}

async fn explore(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<ExploreReq>,
) -> ApiResult<Json<Value>> {
    let _guard = state.action_lock.lock();
    let outcome = gameplay::explore(&state.game, &user.account, &req.kind)?;
    let level_up = outcome.level_up;
    let wallet = state.game.wallet(&user.account)?;
    drop(_guard);
    state.persist()?;
    Ok(Json(json!({
        "result": outcome,
        "level_up": level_up,
        "wallet": wallet,
    })))
}

#[derive(Deserialize)]
struct CraftReq {
    recipe: String,
}

async fn craft(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<CraftReq>,
) -> ApiResult<Json<Value>> {
    let _guard = state.action_lock.lock();
    let outcome = gameplay::craft(&state.game, &user.account, &req.recipe)?;
    drop(_guard);
    state.persist()?;
    Ok(Json(json!({ "result": outcome })))
}

#[derive(Deserialize)]
struct SellReq {
    item: String,
    quantity: u64,
}

async fn sell(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<SellReq>,
) -> ApiResult<Json<Value>> {
    let _guard = state.action_lock.lock();
    let outcome = gameplay::sell(&state.game, &user.account, &req.item, req.quantity)?;
    drop(_guard);
    state.persist()?;
    Ok(Json(json!({ "result": outcome })))
}

async fn upgrade(State(state): State<Arc<AppState>>, user: AuthUser) -> ApiResult<Json<Value>> {
    let _guard = state.action_lock.lock();
    let outcome = gameplay::upgrade_inventory(&state.game, &user.account)?;
    drop(_guard);
    state.persist()?;
    Ok(Json(json!({ "result": outcome })))
}

#[derive(Deserialize)]
struct TravelReq {
    region: String,
}

async fn travel(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<TravelReq>,
) -> ApiResult<Json<Value>> {
    let _guard = state.action_lock.lock();
    let account = state
        .game
        .set_account_region(&user.account, req.region.trim())?;
    drop(_guard);
    state.persist()?;
    Ok(Json(json!({ "account": account })))
}

// ---- rooms --------------------------------------------------------------

#[derive(Deserialize)]
struct RoomCreateReq {
    name: String,
    #[serde(default)]
    capacity: Option<usize>,
    #[serde(default)]
    topic: Option<String>,
}

async fn room_create(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<RoomCreateReq>,
) -> ApiResult<Json<Value>> {
    let account = state.game.account(&user.account)?;
    let mut metadata = BTreeMap::new();
    if let Some(topic) = req.topic.filter(|topic| !topic.trim().is_empty()) {
        metadata.insert("topic".to_string(), topic.trim().to_string());
    }
    let room = state.game.create_room(
        &user.account,
        &req.name,
        &account.region,
        req.capacity.unwrap_or(8),
        metadata,
    )?;
    let _ = state.game.send_system_message(
        ChatChannel::Room(room.id.clone()),
        &format!("{} 创建了房间", account.display_name),
    );
    state.persist()?;
    Ok(Json(json!({ "room": room })))
}

#[derive(Deserialize)]
struct RoomReq {
    room: String,
}

async fn room_join(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<RoomReq>,
) -> ApiResult<Json<Value>> {
    let account = state.game.account(&user.account)?;
    let room_id: RoomId = req.room.into();
    let room = state.game.join_room(&user.account, &room_id)?;
    let _ = state.game.send_system_message(
        ChatChannel::Room(room_id),
        &format!("{} 加入了房间", account.display_name),
    );
    state.persist()?;
    Ok(Json(json!({ "room": room })))
}

async fn room_leave(State(state): State<Arc<AppState>>, user: AuthUser) -> ApiResult<Json<Value>> {
    let account = state.game.account(&user.account)?;
    let room_id = state.game.room_id_of(&user.account)?;
    state.game.leave_room(&user.account)?;
    if let Some(room_id) = room_id {
        let _ = state.game.send_system_message(
            ChatChannel::Room(room_id),
            &format!("{} 离开了房间", account.display_name),
        );
    }
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

async fn room_close(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<RoomReq>,
) -> ApiResult<Json<Value>> {
    let room_id: RoomId = req.room.into();
    let _ = state
        .game
        .send_system_message(ChatChannel::Room(room_id.clone()), "房主关闭了房间");
    let room = state.game.close_room(&user.account, &room_id)?;
    state.persist()?;
    Ok(Json(json!({ "room": room })))
}

#[derive(Deserialize)]
struct KickReq {
    room: String,
    account: String,
}

async fn room_kick(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<KickReq>,
) -> ApiResult<Json<Value>> {
    let room_id: RoomId = req.room.into();
    let target: AccountId = req.account.into();
    let target_name = state
        .game
        .account(&target)
        .map(|account| account.display_name)?;
    state
        .game
        .kick_from_room(&user.account, &room_id, &target)?;
    let _ = state.game.send_system_message(
        ChatChannel::Room(room_id),
        &format!("{target_name} 被房主请出了房间"),
    );
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct MetadataReq {
    room: String,
    key: String,
    #[serde(default)]
    value: String,
}

async fn room_metadata(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<MetadataReq>,
) -> ApiResult<Json<Value>> {
    state
        .game
        .update_room_metadata(&user.account, &req.room.into(), req.key, req.value)?;
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct MetadataRemoveReq {
    room: String,
    key: String,
}

async fn room_metadata_remove(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<MetadataRemoveReq>,
) -> ApiResult<Json<Value>> {
    let removed = state
        .game
        .remove_room_metadata(&user.account, &req.room.into(), &req.key)?;
    state.persist()?;
    Ok(Json(json!({ "removed": removed })))
}

// ---- chat ---------------------------------------------------------------

#[derive(Deserialize)]
struct ChatSendReq {
    channel: String,
    body: String,
    #[serde(default)]
    metadata: Option<BTreeMap<String, String>>,
}

async fn chat_send(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<ChatSendReq>,
) -> ApiResult<Json<Value>> {
    let channel = gameplay::user_channel(&state.game, &user.account, &req.channel)?;
    let message = state.game.send_message_with_metadata(
        &user.account,
        channel,
        &req.body,
        req.metadata.unwrap_or_default(),
    )?;
    let outcome = state.game.record_event(&user.account, "messages", 1)?;
    state.persist()?;
    Ok(Json(json!({ "message": message, "outcome": outcome })))
}

#[derive(Deserialize)]
struct InviteTradeReq {
    account: String,
}

async fn chat_invite_trade(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<InviteTradeReq>,
) -> ApiResult<Json<Value>> {
    let partner: AccountId = req.account.into();
    let me = state.game.account(&user.account)?;
    let _guard = state.action_lock.lock();
    let trade = state.game.create_trade(&user.account, &partner)?;
    let metadata = BTreeMap::from([
        ("kind".to_string(), "trade_invite".to_string()),
        ("trade".to_string(), trade.id.to_string()),
    ]);
    if let Err(error) = state.game.send_message_with_metadata(
        &user.account,
        ChatChannel::private(user.account.clone(), partner.clone()),
        &format!("{} 邀请你进行交易", me.display_name),
        metadata,
    ) {
        let _ = state.game.cancel_trade(&user.account, &trade.id);
        return Err(error.into());
    }
    drop(_guard);
    state.persist()?;
    Ok(Json(json!({ "trade": trade })))
}

// ---- shops --------------------------------------------------------------

#[derive(Deserialize)]
struct BuyReq {
    shop: String,
    offer: String,
    #[serde(default)]
    quantity: Option<u64>,
}

async fn shop_buy(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<BuyReq>,
) -> ApiResult<Json<Value>> {
    let quantity = req.quantity.unwrap_or(1);
    let _guard = state.action_lock.lock();
    let receipt =
        state
            .game
            .purchase(&user.account, &req.shop.into(), &req.offer.into(), quantity)?;
    let outcome = state
        .game
        .record_event(&user.account, "purchases", quantity)?;
    let level_up = gameplay::sync_level(&state.game, &user.account)?;
    drop(_guard);
    state.persist()?;
    Ok(Json(json!({
        "receipt": receipt,
        "outcome": outcome,
        "level_up": level_up,
    })))
}

// ---- trades -------------------------------------------------------------

#[derive(Deserialize)]
struct TradeCreateReq {
    account: String,
}

async fn trade_create(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<TradeCreateReq>,
) -> ApiResult<Json<Value>> {
    let trade = state
        .game
        .create_trade(&user.account, &req.account.into())?;
    state.persist()?;
    Ok(Json(json!({ "trade": trade })))
}

#[derive(Deserialize)]
struct TradeSideReq {
    trade: String,
    #[serde(default)]
    items: BTreeMap<String, u64>,
    #[serde(default)]
    currencies: BTreeMap<String, u64>,
}

async fn trade_side(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<TradeSideReq>,
) -> ApiResult<Json<Value>> {
    let items: BTreeMap<ItemId, u64> = req
        .items
        .into_iter()
        .map(|(item, quantity)| (item.into(), quantity))
        .collect();
    let currencies: BTreeMap<CurrencyId, u64> = req
        .currencies
        .into_iter()
        .map(|(currency, amount)| (currency.into(), amount))
        .collect();
    let trade = state.game.set_trade_side(
        &user.account,
        &req.trade.into(),
        TradeSide::new()
            .with_items(items)
            .with_currencies(currencies),
    )?;
    state.persist()?;
    Ok(Json(json!({ "trade": trade })))
}

#[derive(Deserialize)]
struct TradeIdReq {
    trade: String,
}

async fn trade_confirm(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<TradeIdReq>,
) -> ApiResult<Json<Value>> {
    let trade = state.game.confirm_trade(&user.account, &req.trade.into())?;
    if trade.state == TradeState::Completed {
        let _ = state.game.record_event(&trade.initiator, "trades", 1);
        let _ = state.game.record_event(&trade.partner, "trades", 1);
        let _ = state.game.send_system_message(
            ChatChannel::private(trade.initiator.clone(), trade.partner.clone()),
            "交易已完成，资产已原子结算。",
        );
    }
    state.persist()?;
    Ok(Json(json!({ "trade": trade })))
}

async fn trade_cancel(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<TradeIdReq>,
) -> ApiResult<Json<Value>> {
    let trade = state.game.cancel_trade(&user.account, &req.trade.into())?;
    state.persist()?;
    Ok(Json(json!({ "trade": trade })))
}

// ---- friends ------------------------------------------------------------

#[derive(Deserialize)]
struct FriendReq {
    account: String,
    #[serde(default)]
    message: Option<String>,
}

async fn friend_request(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<FriendReq>,
) -> ApiResult<Json<Value>> {
    let target: AccountId = req.account.into();
    let request = state
        .game
        .send_friend_request(&user.account, &target, req.message.as_deref())?;
    let me = state.game.account(&user.account)?;
    let _ = state.game.send_system_message(
        ChatChannel::private(user.account.clone(), target),
        &format!("{} 发来了好友申请", me.display_name),
    );
    state.persist()?;
    Ok(Json(json!({ "request": request })))
}

#[derive(Deserialize)]
struct FriendActionReq {
    account: String,
}

async fn friend_accept(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<FriendActionReq>,
) -> ApiResult<Json<Value>> {
    let sender: AccountId = req.account.into();
    state.game.accept_friend_request(&user.account, &sender)?;
    let me = state.game.account(&user.account)?;
    let _ = state.game.send_system_message(
        ChatChannel::private(user.account.clone(), sender),
        &format!("你和 {} 成为了好友", me.display_name),
    );
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

async fn friend_decline(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<FriendActionReq>,
) -> ApiResult<Json<Value>> {
    state
        .game
        .decline_friend_request(&user.account, &req.account.into())?;
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

async fn friend_cancel(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<FriendActionReq>,
) -> ApiResult<Json<Value>> {
    state
        .game
        .cancel_friend_request(&user.account, &req.account.into())?;
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

async fn friend_remove(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<FriendActionReq>,
) -> ApiResult<Json<Value>> {
    let removed = state
        .game
        .remove_friend(&user.account, &req.account.into())?;
    state.persist()?;
    Ok(Json(json!({ "removed": removed })))
}

async fn friend_block(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<FriendActionReq>,
) -> ApiResult<Json<Value>> {
    state
        .game
        .block_account(&user.account, &req.account.into())?;
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

async fn friend_unblock(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<FriendActionReq>,
) -> ApiResult<Json<Value>> {
    state
        .game
        .unblock_account(&user.account, &req.account.into())?;
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

// ---- profile / cosmetics ------------------------------------------------

#[derive(Deserialize)]
struct ProfileReq {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    metadata: Option<BTreeMap<String, String>>,
    #[serde(default)]
    remove_metadata: Option<Vec<String>>,
}

async fn update_profile(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<ProfileReq>,
) -> ApiResult<Json<Value>> {
    if let Some(name) = req.display_name {
        state.game.update_display_name(&user.account, &name)?;
    }
    if let Some(metadata) = req.metadata {
        for (key, value) in metadata {
            state.game.set_account_metadata(&user.account, key, value)?;
        }
    }
    if let Some(keys) = req.remove_metadata {
        for key in keys {
            state.game.remove_account_metadata(&user.account, &key)?;
        }
    }
    let account = state.game.account(&user.account)?;
    state.persist()?;
    Ok(Json(json!({ "account": account })))
}

#[derive(Deserialize)]
struct AvatarReq {
    #[serde(default)]
    image: Option<Value>,
    #[serde(default)]
    title: Option<Value>,
    #[serde(default)]
    frame: Option<Value>,
    #[serde(default)]
    badge: Option<Value>,
}

fn optional_string(value: Value, field: &str) -> ApiResult<Option<String>> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_str()
        .map(|value| Some(value.to_owned()))
        .ok_or_else(|| ApiError::bad_request(format!("{field} 必须是字符串或 null")))
}

async fn update_avatar(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<AvatarReq>,
) -> ApiResult<Json<Value>> {
    let account = state.game.account(&user.account)?;
    let mut avatar = account.avatar;
    if let Some(value) = req.image {
        avatar.image = optional_string(value, "image")?;
    }
    if let Some(value) = req.title {
        avatar.title = optional_string(value, "title")?;
    }
    if let Some(value) = req.frame {
        avatar.frame = optional_string(value, "frame")?.map(Into::into);
    }
    if let Some(value) = req.badge {
        avatar.badge = optional_string(value, "badge")?.map(Into::into);
    }
    let avatar = state.game.update_avatar(&user.account, avatar)?;
    state.persist()?;
    Ok(Json(json!({ "avatar": avatar })))
}

#[derive(Deserialize)]
struct SkinReq {
    skin: String,
}

async fn equip_skin(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<SkinReq>,
) -> ApiResult<Json<Value>> {
    let skin: SkinId = req.skin.into();
    let previous = state.game.equip_skin(&user.account, &skin)?;
    let definition = state
        .game
        .skins()
        .into_iter()
        .find(|definition| definition.id == skin);
    let mut account = state.game.account(&user.account)?;
    if let Some(definition) = &definition {
        match definition.category {
            SkinCategory::Frame => account.avatar.frame = Some(skin.clone()),
            SkinCategory::Badge => account.avatar.badge = Some(skin.clone()),
            _ => {}
        }
        if matches!(
            definition.category,
            SkinCategory::Frame | SkinCategory::Badge
        ) {
            let avatar = state.game.update_avatar(&user.account, account.avatar)?;
            account.avatar = avatar;
        }
    }
    state.persist()?;
    Ok(Json(json!({
        "previous": previous,
        "equipped": state.game.equipped_skins(&user.account)?,
        "avatar": account.avatar,
    })))
}

#[derive(Deserialize)]
struct CategoryReq {
    category: String,
}

async fn unequip_skin(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<CategoryReq>,
) -> ApiResult<Json<Value>> {
    let category: SkinCategory = parse_enum(&req.category, "皮肤分类")?;
    let removed = state.game.unequip_skin(&user.account, &category)?;
    let mut account = state.game.account(&user.account)?;
    match category {
        SkinCategory::Frame => account.avatar.frame = None,
        SkinCategory::Badge => account.avatar.badge = None,
        _ => {}
    }
    if matches!(category, SkinCategory::Frame | SkinCategory::Badge) {
        account.avatar = state.game.update_avatar(&user.account, account.avatar)?;
    }
    state.persist()?;
    Ok(Json(json!({
        "removed": removed,
        "equipped": state.game.equipped_skins(&user.account)?,
        "avatar": account.avatar,
    })))
}

// ---- progression --------------------------------------------------------

#[derive(Deserialize)]
struct IdReq {
    id: String,
}

async fn announcement_read(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<IdReq>,
) -> ApiResult<Json<Value>> {
    state
        .game
        .mark_announcement_read(&user.account, &req.id.into())?;
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

async fn achievement_claim(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<IdReq>,
) -> ApiResult<Json<Value>> {
    let reward = state
        .game
        .claim_achievement(&user.account, &req.id.into())?;
    let level_up = gameplay::sync_level(&state.game, &user.account)?;
    state.persist()?;
    Ok(Json(json!({ "reward": reward, "level_up": level_up })))
}

#[derive(Deserialize)]
struct TaskIdReq {
    task: String,
}

async fn task_accept(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<TaskIdReq>,
) -> ApiResult<Json<Value>> {
    let view = state.game.accept_task(&user.account, &req.task.into())?;
    state.persist()?;
    Ok(Json(json!({ "task": view })))
}

async fn task_abandon(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<TaskIdReq>,
) -> ApiResult<Json<Value>> {
    state.game.abandon_task(&user.account, &req.task.into())?;
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

async fn task_claim(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<TaskIdReq>,
) -> ApiResult<Json<Value>> {
    let reward = state.game.claim_task(&user.account, &req.task.into())?;
    let level_up = gameplay::sync_level(&state.game, &user.account)?;
    state.persist()?;
    Ok(Json(json!({ "reward": reward, "level_up": level_up })))
}

#[derive(Deserialize)]
struct CollectionClaimReq {
    collection: String,
    milestone: usize,
}

async fn collection_claim(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<CollectionClaimReq>,
) -> ApiResult<Json<Value>> {
    let reward = state.game.claim_collection_milestone(
        &user.account,
        &req.collection.into(),
        req.milestone,
    )?;
    let level_up = gameplay::sync_level(&state.game, &user.account)?;
    state.persist()?;
    Ok(Json(json!({ "reward": reward, "level_up": level_up })))
}

// ---- NPC ----------------------------------------------------------------

#[derive(Deserialize)]
struct NpcSayReq {
    npc: String,
    message: String,
}

fn map_npc_error(error: NpcError) -> ApiError {
    match error {
        NpcError::Game(error) => error.into(),
        NpcError::NoAgent(npc) => ApiError::not_found(format!("NPC {npc} 没有接入 Agent")),
        NpcError::Agent(message) => ApiError::bad_request(format!("NPC Agent 错误: {message}")),
    }
}

async fn npc_say(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<NpcSayReq>,
) -> ApiResult<Json<Value>> {
    let turn = state
        .npcs
        .converse(&state.game, &req.npc.into(), &user.account, &req.message)
        .await
        .map_err(map_npc_error)?;
    let outcome = state.game.record_event(&user.account, "messages", 1)?;
    let level_up = gameplay::sync_level(&state.game, &user.account)?;
    state.persist()?;
    Ok(Json(json!({
        "turn": turn,
        "outcome": outcome,
        "level_up": level_up,
    })))
}

#[derive(Deserialize)]
struct NpcPromptReq {
    npc: String,
    #[serde(default)]
    message: String,
}

async fn npc_prompt(
    State(state): State<Arc<AppState>>,
    user: AuthUser,
    Json(req): Json<NpcPromptReq>,
) -> ApiResult<Json<Value>> {
    let message = if req.message.trim().is_empty() {
        "你好"
    } else {
        req.message.as_str()
    };
    let context = state
        .game
        .npc_context(&req.npc.into(), &user.account, message)?;
    Ok(Json(json!({
        "npc": context.npc.name,
        "channel": context.channel.to_string(),
        "prompt": context.system_prompt(),
    })))
}

// =========================================================================
// Admin
// =========================================================================

async fn admin_overview(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let snapshot = state.game.read_state();
    let online = snapshot
        .accounts
        .values()
        .filter(|account| account.online)
        .count();
    let snapshot_bytes = serde_json::to_vec(&*snapshot)
        .map(|bytes| bytes.len())
        .unwrap_or(0);
    Ok(Json(json!({
        "now_ms": state.game.now_ms(),
        "manual_clock": state.manual_clock.is_some(),
        "accounts": snapshot.accounts.len(),
        "online": online,
        "sessions": state.session_count(),
        "rooms": snapshot.rooms.len(),
        "shops": snapshot.shops.len(),
        "trades": snapshot.trades.len(),
        "chat_channels": snapshot.chat.channel_count(),
        "leaderboards": snapshot.leaderboards.len(),
        "snapshot_bytes": snapshot_bytes,
        "store": state.store.path().display().to_string(),
        "server_started_at_ms": state.started_at_ms,
    })))
}

async fn admin_accounts(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let snapshot = state.game.read_state();
    let accounts: Vec<_> = snapshot.accounts.values().cloned().collect();
    Ok(Json(json!(accounts)))
}

async fn admin_tasks(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    Ok(Json(json!(state.game.task_definitions())))
}

async fn admin_announcements(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    Ok(Json(json!(state.game.announcements())))
}

async fn admin_npcs(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    Ok(Json(json!(state.game.npcs())))
}

async fn admin_snapshot(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<GameState>> {
    state.require_admin(&headers)?;
    Ok(Json(state.game.snapshot()))
}

#[derive(Deserialize)]
struct AdminAnnouncementReq {
    title: String,
    body: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    priority: Option<i32>,
    #[serde(default)]
    regions: Option<Vec<String>>,
    #[serde(default)]
    starts_in_ms: Option<i64>,
    #[serde(default)]
    ends_in_ms: Option<i64>,
    #[serde(default)]
    enabled: Option<bool>,
}

async fn admin_announcement(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<AdminAnnouncementReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let now = state.game.now_ms();
    let mut draft = AnnouncementDraft::new(req.title, req.body);
    if let Some(kind) = req.kind {
        draft.kind = parse_enum(&kind, "公告类型")?;
    }
    if let Some(priority) = req.priority {
        draft.priority = priority;
    }
    if let Some(regions) = req.regions {
        draft = draft.with_regions(regions.into_iter().map(RegionId::from));
    }
    draft.starts_at_ms = req.starts_in_ms.map(|offset| now.saturating_add(offset));
    draft.ends_at_ms = req.ends_in_ms.map(|offset| now.saturating_add(offset));
    if let Some(enabled) = req.enabled {
        draft.enabled = enabled;
    }
    let announcement = state.game.publish_announcement(draft)?;
    state.persist()?;
    Ok(Json(json!({ "announcement": announcement })))
}

#[derive(Deserialize)]
struct AnnouncementEnabledReq {
    id: String,
    enabled: bool,
}

async fn admin_announcement_enabled(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<AnnouncementEnabledReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    state
        .game
        .set_announcement_enabled(&req.id.into(), req.enabled)?;
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

async fn admin_announcement_remove(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<IdReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let removed = state.game.remove_announcement(&req.id.into());
    state.persist()?;
    Ok(Json(json!({ "removed": removed })))
}

#[derive(Deserialize)]
struct RegionStatusReq {
    region: String,
    status: String,
}

async fn admin_region_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<RegionStatusReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let status: RegionStatus = parse_enum(&req.status, "区域状态")?;
    let region = state.game.set_region_status(&req.region.into(), status)?;
    state.persist()?;
    Ok(Json(json!({ "region": region })))
}

#[derive(Deserialize)]
struct LeaderboardResetReq {
    leaderboard: String,
}

async fn admin_leaderboard_reset(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<LeaderboardResetReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let season = state
        .game
        .reset_leaderboard_season(&req.leaderboard.into())?;
    state.persist()?;
    Ok(Json(json!({ "season": season })))
}

#[derive(Deserialize)]
struct ChatDeleteReq {
    channel: String,
    message: String,
}

async fn admin_chat_delete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ChatDeleteReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let channel = gameplay::admin_channel(&req.channel)?;
    state.game.delete_message(&channel, &req.message.into())?;
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ChatClearReq {
    channel: String,
}

async fn admin_chat_clear(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ChatClearReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let channel = gameplay::admin_channel(&req.channel)?;
    let cleared = state.game.clear_channel(&channel);
    state.persist()?;
    Ok(Json(json!({ "cleared": cleared })))
}

#[derive(Deserialize)]
struct GrantReq {
    account: String,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    currency_amount: Option<u64>,
    #[serde(default)]
    item: Option<String>,
    #[serde(default)]
    item_quantity: Option<u64>,
    #[serde(default)]
    resource: Option<String>,
    #[serde(default)]
    resource_amount: Option<u64>,
    #[serde(default)]
    skin: Option<String>,
    #[serde(default)]
    xp: Option<u64>,
}

async fn admin_grant(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<GrantReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let account: AccountId = req.account.into();
    let mut touched = false;
    if let (Some(currency), Some(amount)) = (req.currency, req.currency_amount) {
        state
            .game
            .add_currency(&account, &currency.into(), amount)?;
        touched = true;
    }
    if let (Some(item), Some(quantity)) = (req.item, req.item_quantity) {
        state.game.add_item(&account, &item.into(), quantity)?;
        touched = true;
    }
    if let (Some(resource), Some(amount)) = (req.resource, req.resource_amount) {
        state
            .game
            .add_resource(&account, &resource.into(), amount)?;
        touched = true;
    }
    if let Some(skin) = req.skin {
        state.game.grant_skin(&account, &skin.into())?;
        touched = true;
    }
    if let Some(amount) = req.xp {
        state.game.add_xp(&account, amount)?;
        touched = true;
    }
    if !touched {
        return Err(ApiError::bad_request("至少提供一个发放字段"));
    }
    let level_up = gameplay::sync_level(&state.game, &account)?;
    state.persist()?;
    Ok(Json(json!({
        "account": state.game.account(&account)?,
        "wallet": state.game.wallet(&account)?,
        "level_up": level_up,
    })))
}

#[derive(Deserialize)]
struct CapacityReq {
    account: String,
    capacity: usize,
}

async fn admin_capacity(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CapacityReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let inventory = state
        .game
        .set_inventory_capacity(&req.account.into(), req.capacity)?;
    state.persist()?;
    Ok(Json(json!({ "inventory": inventory })))
}

#[derive(Deserialize)]
struct NpcEnabledReq {
    npc: String,
    enabled: bool,
}

async fn admin_npc_enabled(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<NpcEnabledReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    state.game.set_npc_enabled(&req.npc.into(), req.enabled)?;
    state.persist()?;
    Ok(Json(json!({ "ok": true })))
}

async fn admin_npc_remove(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<IdReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let removed = state.game.remove_npc(&req.id.into());
    state.persist()?;
    Ok(Json(json!({ "removed": removed })))
}

#[derive(Deserialize)]
struct OfferReq {
    shop: String,
    offer: String,
}

async fn admin_offer_remove(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<OfferReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let removed = state
        .game
        .remove_offer(&req.shop.into(), &req.offer.into())?;
    state.persist()?;
    Ok(Json(json!({ "removed": removed })))
}

#[derive(Deserialize)]
struct ClockReq {
    #[serde(default)]
    ms: i64,
}

async fn admin_clock_advance(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ClockReq>,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let Some(clock) = &state.manual_clock else {
        return Err(ApiError::bad_request(
            "服务器未启用 ManualClock（TAIXU_MANUAL_CLOCK=1）",
        ));
    };
    let now = clock.advance(req.ms);
    state.persist()?;
    Ok(Json(json!({ "now_ms": now })))
}

async fn admin_trade_expire(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    let expired = state.game.expire_trades();
    state.persist()?;
    Ok(Json(json!({ "expired": expired })))
}

async fn admin_save(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    state.require_admin(&headers)?;
    state.persist()?;
    Ok(Json(json!({ "ok": true, "now_ms": state.now_ms() })))
}

// Keep the import used even if extractor trait changes.
#[allow(dead_code)]
fn _assert_extractor_impl<T: FromRequestParts<Arc<AppState>>>() {}

#[allow(dead_code)]
fn _assert_game_error_is_used(_: GameError) {}
