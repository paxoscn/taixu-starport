//! Player-facing gameplay rules that sit *above* the library.
//!
//! Taixu intentionally owns domain invariants but not game design. Everything
//! here is host code: energy costs, loot tables, XP curves, leaderboard score
//! formulas, and the composite flows (spend → grant → record) that the library
//! does not expose as one primitive. Composite flows are serialized by
//! `AppState::action_lock` and refund resources on delivery failure.

use rand::Rng;
use serde::Serialize;
use taixu::prelude::*;
use taixu::GameError;

use crate::error::{ApiError, ApiResult};

pub const HIDDEN_TASK: &str = "secret_trail";

#[derive(Clone, Copy, Debug, Serialize)]
pub struct CurrencyMeta {
    pub id: &'static str,
    pub name: &'static str,
    pub symbol: &'static str,
    pub premium: bool,
}

pub fn currency_meta() -> Vec<CurrencyMeta> {
    vec![
        CurrencyMeta {
            id: "gold",
            name: "金币",
            symbol: "◈",
            premium: false,
        },
        CurrencyMeta {
            id: "gems",
            name: "星钻",
            symbol: "◆",
            premium: true,
        },
        CurrencyMeta {
            id: "astral",
            name: "星尘券",
            symbol: "✦",
            premium: false,
        },
    ]
}

pub fn metric_label(metric: &str) -> &str {
    match metric {
        "expeditions" => "远征次数",
        "gathers" => "采集次数",
        "boss" => "首领战次数",
        "secret_found" => "太虚秘密",
        "trades" => "交易次数",
        "messages" => "发言次数",
        "purchases" => "购买次数",
        "crafts" => "合成次数",
        "sales" => "出售次数",
        "upgrades" => "背包扩容次数",
        other => other,
    }
}

pub fn level_from_xp(xp: u64) -> u32 {
    1 + (xp / 100).isqrt() as u32
}

/// Host-defined activities (explore/craft/sell/upgrade) also honor Taixu's
/// `Closed` region semantics. The library gates its own built-ins; this guard
/// covers actions that live entirely in the host.
pub fn ensure_region_active(game: &Game, account: &AccountId) -> ApiResult<()> {
    let info = game.account(account)?;
    let region = game.region(&info.region)?;
    if region.status == RegionStatus::Closed {
        return Err(ApiError::forbidden(format!(
            "{} 已关闭，暂时无法进行该操作",
            region.name
        )));
    }
    Ok(())
}

/// `(current_level_floor, next_level_floor)` in XP.
pub fn xp_bounds(level: u32) -> (u64, u64) {
    let level = u64::from(level.max(1));
    let base = (level - 1) * (level - 1) * 100;
    let next = level * level * 100;
    (base, next)
}

/// Recompute the host-side level from XP. Taixu stores level and XP but does
/// not impose a curve; the host decides when to call `set_level`.
pub fn sync_level(game: &Game, account: &AccountId) -> taixu::Result<Option<u32>> {
    let info = game.account(account)?;
    let target = level_from_xp(info.xp);
    let level_up = if target > info.level {
        game.set_level(account, target)?;
        let _ = game.send_system_message(
            ChatChannel::Region(info.region.clone()),
            &format!("✦ {} 晋升至 {} 级", info.display_name, target),
        );
        Some(target)
    } else {
        None
    };
    // XP can change without a level-up; keep the power board in sync either way.
    recompute_power(game, account)?;
    Ok(level_up)
}

pub fn recompute_power(game: &Game, account: &AccountId) -> taixu::Result<i64> {
    let info = game.account(account)?;
    let power = i64::from(info.level) * 100 + (info.xp / 10) as i64;
    game.set_leaderboard_score(&"power".into(), account, power)?;
    Ok(power)
}

fn set_best_low_score(
    game: &Game,
    board: &LeaderboardId,
    account: &AccountId,
    score: i64,
) -> taixu::Result<()> {
    let current = game.leaderboard_score(board, account)?;
    if current.is_none_or(|current| score < current) {
        game.set_leaderboard_score(board, account, score)?;
        recompute_power(game, account)?;
    }
    Ok(())
}

// =========================================================================
// Exploration
// =========================================================================

#[derive(Clone, Copy, Debug)]
pub enum ExploreKind {
    Expedition,
    Gather,
    Boss,
    Secret,
}

impl ExploreKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "expedition" => Some(Self::Expedition),
            "gather" => Some(Self::Gather),
            "boss" => Some(Self::Boss),
            "secret" => Some(Self::Secret),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Expedition => "expedition",
            Self::Gather => "gather",
            Self::Boss => "boss",
            Self::Secret => "secret",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Expedition => "深空远征",
            Self::Gather => "近地采集",
            Self::Boss => "裂隙首领",
            Self::Secret => "太虚窥探",
        }
    }

    pub fn energy_cost(self) -> u64 {
        match self {
            Self::Expedition => 10,
            Self::Gather => 5,
            Self::Boss => 25,
            Self::Secret => 15,
        }
    }

    pub fn focus_cost(self) -> u64 {
        match self {
            Self::Secret => 3,
            _ => 0,
        }
    }

    pub fn metric(self) -> &'static str {
        match self {
            Self::Expedition => "expeditions",
            Self::Gather => "gathers",
            Self::Boss => "boss",
            Self::Secret => "secret_found",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ExploreOutcome {
    pub kind: String,
    pub label: String,
    pub narrative: String,
    pub reward: Reward,
    pub outcome: MetricOutcome,
    pub level_up: Option<u32>,
    pub secret: bool,
    pub speed_secs: Option<i64>,
}

pub fn explore(game: &Game, account: &AccountId, raw_kind: &str) -> ApiResult<ExploreOutcome> {
    ensure_region_active(game, account)?;
    let kind =
        ExploreKind::parse(raw_kind).ok_or_else(|| ApiError::bad_request("未知的行动类型"))?;
    let energy: ResourceId = "energy".into();
    let focus: ResourceId = "focus".into();

    let energy_balance = game.resource_balance(account, &energy)?;
    if energy_balance < kind.energy_cost() {
        return Err(ApiError::conflict(format!(
            "体力不足：需要 {}，当前 {}",
            kind.energy_cost(),
            energy_balance
        )));
    }
    if kind.focus_cost() > 0 {
        let focus_balance = game.resource_balance(account, &focus)?;
        if focus_balance < kind.focus_cost() {
            return Err(ApiError::conflict(format!(
                "专注不足：需要 {}，当前 {}",
                kind.focus_cost(),
                focus_balance
            )));
        }
    }

    game.spend_resource(account, &energy, kind.energy_cost())?;
    if kind.focus_cost() > 0 {
        if let Err(error) = game.spend_resource(account, &focus, kind.focus_cost()) {
            let _ = game.add_resource(account, &energy, kind.energy_cost());
            return Err(error.into());
        }
    }

    let mut rng = rand::rng();
    let reward = roll_reward(kind, &mut rng);
    if let Err(error) = game.grant_reward(account, &reward) {
        // Compensate: the composite flow is host-owned, so delivery failure
        // must not consume stamina.
        let _ = game.add_resource(account, &energy, kind.energy_cost());
        let _ = game.add_resource(account, &focus, kind.focus_cost());
        return Err(error.into());
    }

    let secret = matches!(kind, ExploreKind::Secret);
    if secret {
        // Hidden tasks are discoverable by id rather than listed; the host
        // reveals one when the secret event happens.
        match game.accept_task(account, &HIDDEN_TASK.into()) {
            Ok(_)
            | Err(GameError::TaskAlreadyAccepted(_))
            | Err(GameError::TaskAlreadyCompleted(_))
            | Err(GameError::TaskAlreadyClaimed(_))
            | Err(GameError::TaskOnCooldown { .. })
            | Err(GameError::TaskLocked(_)) => {}
            Err(error) => return Err(error.into()),
        }
    }

    let outcome = game.record_event(account, kind.metric(), 1)?;
    let speed_secs = if matches!(kind, ExploreKind::Expedition) {
        let secs = rng.random_range(12..=45i64);
        set_best_low_score(game, &"speed".into(), account, secs)?;
        Some(secs)
    } else {
        None
    };
    let level_up = sync_level(game, account)?;

    let narrative = match kind {
        ExploreKind::Expedition => {
            format!(
                "你穿过极光航道，带回了一箱战利品。用时 {} 秒。",
                speed_secs.unwrap_or(0)
            )
        }
        ExploreKind::Gather => "你在近地尘埃带采集到了可用资源。".to_string(),
        ExploreKind::Boss => "裂隙首领被击退，遗物在残骸中闪烁。".to_string(),
        ExploreKind::Secret => {
            "你听见了太虚深处的低语，一段被隐藏的轨迹浮现在任务板上。".to_string()
        }
    };

    Ok(ExploreOutcome {
        kind: kind.key().into(),
        label: kind.label().into(),
        narrative,
        reward,
        outcome,
        level_up,
        secret,
        speed_secs,
    })
}

fn roll_reward(kind: ExploreKind, rng: &mut impl Rng) -> Reward {
    let mut reward = Reward::new();
    match kind {
        ExploreKind::Expedition => {
            reward = reward
                .currency("gold", rng.random_range(12..=28u64))
                .xp(rng.random_range(12..=24u64))
                .item("ore", rng.random_range(1..=2u64));
            if rng.random_bool(0.25) {
                reward = reward.item("potion", 1);
            }
            if rng.random_bool(0.08) {
                reward = reward.item("rift_shard", 1);
            }
            if rng.random_bool(0.02) {
                reward = reward.item("relic", 1);
            }
        }
        ExploreKind::Gather => {
            reward = reward
                .currency("gold", rng.random_range(4..=10u64))
                .xp(rng.random_range(5..=10u64))
                .item("ore", rng.random_range(2..=4u64));
            if rng.random_bool(0.30) {
                reward = reward.item("potion", 1);
            }
            if rng.random_bool(0.05) {
                reward = reward.item("rift_shard", 1);
            }
        }
        ExploreKind::Boss => {
            reward = reward
                .currency("gold", rng.random_range(40..=80u64))
                .xp(rng.random_range(30..=60u64))
                .item("rift_shard", 1);
            if rng.random_bool(0.10) {
                reward = reward.item("relic", 1);
            }
            if rng.random_bool(0.02) {
                reward = reward.item("trophy", 1);
            }
        }
        ExploreKind::Secret => {
            reward = reward
                .currency("gold", rng.random_range(20..=35u64))
                .xp(rng.random_range(15..=25u64));
            if rng.random_bool(0.15) {
                reward = reward.item("rift_shard", 1);
            }
            if rng.random_bool(0.03) {
                reward = reward.item("mythic_core", 1);
            }
        }
    }
    reward
}

// =========================================================================
// Crafting, selling, capacity
// =========================================================================

#[derive(Clone, Debug, Serialize)]
pub struct CraftOutcome {
    pub recipe: String,
    pub reward: Reward,
    pub outcome: MetricOutcome,
    pub level_up: Option<u32>,
    pub message: String,
}

struct Recipe {
    id: &'static str,
    label: &'static str,
    inputs: &'static [(&'static str, u64)],
    reward: Reward,
}

fn recipe(id: &str) -> Option<Recipe> {
    match id {
        "rift_shard" => Some(Recipe {
            id: "rift_shard",
            label: "裂隙碎片",
            inputs: &[("ore", 3)],
            reward: Reward::new().item("rift_shard", 1).xp(8),
        }),
        "relic" => Some(Recipe {
            id: "relic",
            label: "遗物",
            inputs: &[("rift_shard", 5), ("potion", 2)],
            reward: Reward::new().item("relic", 1).xp(40),
        }),
        _ => None,
    }
}

pub fn craft(game: &Game, account: &AccountId, recipe_id: &str) -> ApiResult<CraftOutcome> {
    ensure_region_active(game, account)?;
    let recipe = recipe(recipe_id).ok_or_else(|| ApiError::bad_request("未知的合成配方"))?;
    for (item, quantity) in recipe.inputs {
        let available = game.item_count(account, &(*item).into())?;
        if available < *quantity {
            return Err(ApiError::conflict(format!(
                "材料不足：{} 需要 {}，当前 {}",
                item, quantity, available
            )));
        }
    }

    let mut removed: Vec<(ItemId, u64)> = Vec::new();
    for (item, quantity) in recipe.inputs {
        let id: ItemId = (*item).into();
        if let Err(error) = game.remove_item(account, &id, *quantity) {
            for (removed_item, removed_quantity) in removed {
                let _ = game.add_item(account, &removed_item, removed_quantity);
            }
            return Err(error.into());
        }
        removed.push((id, *quantity));
    }

    if let Err(error) = game.grant_reward(account, &recipe.reward) {
        for (removed_item, removed_quantity) in removed {
            let _ = game.add_item(account, &removed_item, removed_quantity);
        }
        return Err(error.into());
    }

    let outcome = game.record_event(account, "crafts", 1)?;
    let level_up = sync_level(game, account)?;
    Ok(CraftOutcome {
        recipe: recipe.id.into(),
        reward: recipe.reward,
        outcome,
        level_up,
        message: format!("合成成功：{}。", recipe.label),
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct SellOutcome {
    pub item: String,
    pub quantity: u64,
    pub earned: u64,
    pub outcome: MetricOutcome,
    pub message: String,
}

pub fn sell(game: &Game, account: &AccountId, item: &str, quantity: u64) -> ApiResult<SellOutcome> {
    ensure_region_active(game, account)?;
    if quantity == 0 {
        return Err(ApiError::bad_request("数量必须大于 0"));
    }
    let price = match item {
        "stardust" => 3u64,
        "ore" => 8,
        "potion" => 15,
        "rift_shard" => 60,
        "relic" => 300,
        "trophy" => 500,
        "mythic_core" => 900,
        _ => return Err(ApiError::bad_request("该物品无法出售")),
    };
    let id: ItemId = item.into();
    let available = game.item_count(account, &id)?;
    if available < quantity {
        return Err(ApiError::conflict(format!(
            "持有不足：需要 {}，当前 {}",
            quantity, available
        )));
    }
    let earned = price
        .checked_mul(quantity)
        .ok_or_else(|| ApiError::bad_request("金额溢出"))?;

    game.remove_item(account, &id, quantity)?;
    if let Err(error) = game.add_currency(account, &"gold".into(), earned) {
        let _ = game.add_item(account, &id, quantity);
        return Err(error.into());
    }
    let outcome = game.record_event(account, "sales", quantity)?;
    Ok(SellOutcome {
        item: item.into(),
        quantity,
        earned,
        outcome,
        message: format!("出售 {} x{}，获得 {} 金币。", item, quantity, earned),
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct UpgradeOutcome {
    pub capacity: usize,
    pub cost: u64,
    pub outcome: MetricOutcome,
    pub message: String,
}

pub fn upgrade_inventory(game: &Game, account: &AccountId) -> ApiResult<UpgradeOutcome> {
    ensure_region_active(game, account)?;
    let inventory = game.inventory(account)?;
    let next = inventory.capacity() + 10;
    if next > game.config().max_inventory_capacity {
        return Err(ApiError::conflict("背包已达到最大容量"));
    }
    let cost = (next as u64) * 8;
    game.spend_currency(account, &"gold".into(), cost)?;
    let inventory = match game.set_inventory_capacity(account, next) {
        Ok(inventory) => inventory,
        Err(error) => {
            let _ = game.add_currency(account, &"gold".into(), cost);
            return Err(error.into());
        }
    };
    let outcome = game.record_event(account, "upgrades", 1)?;
    Ok(UpgradeOutcome {
        capacity: inventory.capacity(),
        cost,
        outcome,
        message: format!(
            "背包扩容至 {} 格，花费 {} 金币。",
            inventory.capacity(),
            cost
        ),
    })
}

// =========================================================================
// Channels
// =========================================================================

pub fn user_channel(game: &Game, me: &AccountId, raw: &str) -> ApiResult<ChatChannel> {
    let raw = raw.trim();
    if raw.is_empty() || raw == "global" {
        return Ok(ChatChannel::Global);
    }
    if raw == "region" {
        let account = game.account(me)?;
        return Ok(ChatChannel::Region(account.region));
    }
    if let Some(region) = raw.strip_prefix("region:") {
        if region.is_empty() {
            return Err(ApiError::bad_request("区域频道缺少区域 ID"));
        }
        return Ok(ChatChannel::Region(region.into()));
    }
    if let Some(room) = raw.strip_prefix("room:") {
        if room.is_empty() {
            return Err(ApiError::bad_request("房间频道缺少房间 ID"));
        }
        return Ok(ChatChannel::Room(room.into()));
    }
    if let Some(other) = raw.strip_prefix("dm:") {
        if other.is_empty() {
            return Err(ApiError::bad_request("私聊频道缺少账号 ID"));
        }
        if other == me.as_str() {
            return Err(ApiError::bad_request("不能和自己私聊"));
        }
        return Ok(ChatChannel::private(me.clone(), other.into()));
    }
    Err(ApiError::bad_request(format!("未知聊天频道: {raw}")))
}

/// Operator channel syntax: `global`, `region:<id>`, `room:<id>`,
/// `dm:<account>|<account>`.
pub fn admin_channel(raw: &str) -> ApiResult<ChatChannel> {
    let raw = raw.trim();
    if raw.is_empty() || raw == "global" {
        return Ok(ChatChannel::Global);
    }
    if let Some(region) = raw.strip_prefix("region:") {
        return Ok(ChatChannel::Region(region.into()));
    }
    if let Some(room) = raw.strip_prefix("room:") {
        return Ok(ChatChannel::Room(room.into()));
    }
    if let Some(pair) = raw.strip_prefix("dm:") {
        let (a, b) = pair
            .split_once('|')
            .ok_or_else(|| ApiError::bad_request("运营私聊频道格式为 dm:<a>|<b>"))?;
        if a == b {
            return Err(ApiError::bad_request("私聊双方不能相同"));
        }
        return Ok(ChatChannel::private(a.into(), b.into()));
    }
    Err(ApiError::bad_request(format!("未知运营聊天频道: {raw}")))
}

pub fn channel_value(channel: &ChatChannel, me: &AccountId) -> String {
    match channel {
        ChatChannel::Global => "global".into(),
        ChatChannel::Region(region) => format!("region:{region}"),
        ChatChannel::Room(room) => format!("room:{room}"),
        ChatChannel::Private { a, b } => {
            let other = if a == me { b } else { a };
            format!("dm:{other}")
        }
    }
}
