//! World seed. Every Taixu registry is populated here: regions, items,
//! resources, skins (all categories), shops/offers (windows, stock, limits,
//! region targeting), leaderboards (both score orders), achievements (incl.
//! hidden), tasks (all four objective kinds, prerequisites, level/region/time
//! windows, time limits, once/daily/weekly/cooldown repeats, hidden),
//! collection milestones, announcements (all kinds), and NPC placements.

use std::collections::BTreeMap;
use std::sync::Arc;

use taixu::npc::{NpcDef, NpcRuntime, RuleBasedNpc};
use taixu::prelude::*;

use crate::npc_agent::StarportGuide;

pub const REGION_AURORA: &str = "aurora";
pub const REGION_NEBULA: &str = "nebula";
pub const REGION_DRIFT: &str = "drift";
pub const SYSTEM_ACCOUNT: &str = "system";
pub const TAVERN_ROOM_NAME: &str = "星港酒馆";

const DAY_MS: i64 = 86_400_000;

pub fn seed_world(game: &Game) -> taixu::Result<()> {
    let now = game.now_ms();

    // ---- regions ---------------------------------------------------------
    game.create_region(
        REGION_AURORA,
        "极光星港",
        Some(100),
        Some("wss://aurora.starport.local".into()),
    )?;
    game.create_region(
        REGION_NEBULA,
        "星云前哨",
        Some(12),
        Some("wss://nebula.starport.local".into()),
    )?;
    game.create_region(REGION_DRIFT, "漂流禁区", Some(2), None)?;
    game.set_region_status(&REGION_DRIFT.into(), RegionStatus::Closed)?;

    let system = game.create_account(SYSTEM_ACCOUNT, "星港系统", REGION_AURORA)?;
    game.set_account_metadata(&system.id, "role", "system")?;

    register_catalogs(game)?;
    register_shops(game, now)?;
    register_progression(game, now)?;
    register_social_world(game, now)?;

    Ok(())
}

fn register_catalogs(game: &Game) -> taixu::Result<()> {
    // ---- items (every rarity is represented) ----------------------------
    game.register_item(
        ItemDef::new("stardust", "星尘")
            .with_description("随处可见的发光尘埃，可按斤出售。")
            .with_rarity(Rarity::Common)
            .with_stack_limit(99)
            .with_tag("material"),
    )?;
    game.register_item(
        ItemDef::new("ore", "太虚矿石")
            .with_description("合成裂隙碎片的基础材料。")
            .with_rarity(Rarity::Common)
            .with_stack_limit(99)
            .with_tag("material"),
    )?;
    game.register_item(
        ItemDef::new("potion", "星尘药水")
            .with_description("远征前的一口安心。")
            .with_rarity(Rarity::Uncommon)
            .with_stack_limit(20)
            .with_tag("consumable"),
    )?;
    game.register_item(
        ItemDef::new("rift_shard", "裂隙碎片")
            .with_description("空间裂隙凝结的碎片，可合成遗物。")
            .with_rarity(Rarity::Rare)
            .with_stack_limit(10)
            .with_tag("material"),
    )?;
    game.register_item(
        ItemDef::new("relic", "太虚遗物")
            .with_description("上一纪元留下的唯一遗物。")
            .with_rarity(Rarity::Epic)
            .unique()
            .with_tag("treasure"),
    )?;
    game.register_item(
        ItemDef::new("trophy", "星港奖杯")
            .with_description("集齐星港图鉴者的证明。")
            .with_rarity(Rarity::Legendary)
            .unique()
            .with_tag("treasure"),
    )?;
    game.register_item(
        ItemDef::new("mythic_core", "太虚核心")
            .with_description("只在太虚低语中出现的神话核心。")
            .with_rarity(Rarity::Mythic)
            .unique()
            .with_tag("secret"),
    )?;

    // ---- resources -------------------------------------------------------
    game.register_resource(
        ResourceDef::new("energy", "体力")
            .with_cap(100)
            .with_regen_per_hour(20),
    )?;
    game.register_resource(
        ResourceDef::new("focus", "专注")
            .with_cap(30)
            .with_regen_per_hour(6),
    )?;

    // ---- skins: every SkinCategory + rarity ------------------------------
    game.register_skin(
        SkinDef::new("char_ranger", "星港游侠", SkinCategory::Character)
            .with_description("新手制式探索服。")
            .with_rarity(Rarity::Common),
    )?;
    game.register_skin(
        SkinDef::new("weapon_arc", "电弧长枪", SkinCategory::Weapon)
            .with_description("裂隙边缘泛着蓝光。")
            .with_rarity(Rarity::Rare),
    )?;
    game.register_skin(
        SkinDef::new("avatar_halo", "星环头像", SkinCategory::Avatar)
            .with_description("环绕头像的微光星环。")
            .with_rarity(Rarity::Epic),
    )?;
    game.register_skin(
        SkinDef::new("frame_aurora", "极光边框", SkinCategory::Frame)
            .with_description("会随等级流动的极光边框。")
            .with_rarity(Rarity::Rare),
    )?;
    game.register_skin(
        SkinDef::new("frame_gold", "黄金边框", SkinCategory::Frame)
            .with_description("星穹商店限定。")
            .with_rarity(Rarity::Legendary),
    )?;
    game.register_skin(
        SkinDef::new("badge_founder", "拓荒者徽章", SkinCategory::Badge)
            .with_description("第一批抵达太虚星港的人。")
            .with_rarity(Rarity::Legendary),
    )?;
    game.register_skin(
        SkinDef::new("bg_nebula", "星云背景", SkinCategory::Background)
            .with_description("一张会呼吸的星云。")
            .with_rarity(Rarity::Epic),
    )?;
    game.register_skin(
        SkinDef::new("emote_wave", "挥手表情", SkinCategory::Emote)
            .with_description("向星港的邻居打个招呼。")
            .with_rarity(Rarity::Uncommon),
    )?;
    game.register_skin(
        SkinDef::new("other_halo", "太虚光环", SkinCategory::Other("halo".into()))
            .with_description("自定义分类槽位中的神话光环。")
            .with_rarity(Rarity::Mythic),
    )?;

    Ok(())
}

fn register_shops(game: &Game, now: i64) -> taixu::Result<()> {
    // ---- general store: currency + resource pricing, stock, limits ------
    let mut general =
        Shop::new("general", "星港杂货铺").with_description("金币和体力就能解决大部分问题。");
    general.add_offer(
        ShopOffer::new(
            "supply_pack",
            "远征补给包",
            Cost::new().currency("gold", 80).resource("energy", 10),
            Reward::new().item("potion", 2).item("ore", 2).xp(10),
        )
        .with_description("两瓶药水和两袋矿石。")
        .with_per_account_limit(10)
        .with_stock(100)
        .with_tag("supply"),
    )?;
    general.add_offer(
        ShopOffer::new(
            "ore_crate",
            "矿石箱",
            Cost::new().currency("gold", 50),
            Reward::new().item("ore", 5),
        )
        .with_per_account_limit(5),
    )?;
    general.add_offer(
        ShopOffer::new(
            "energy_cell",
            "体力电池",
            Cost::new().currency("gold", 60),
            Reward::new().resource("energy", 25),
        )
        .with_per_account_limit(10),
    )?;
    general.add_offer(
        ShopOffer::new(
            "mystery_box",
            "裂隙盲盒",
            Cost::new().currency("gold", 150).resource("focus", 5),
            Reward::new().item("rift_shard", 1).xp(25),
        )
        .with_description("必出 1 枚裂隙碎片。")
        .with_per_account_limit(3)
        .with_stock(20)
        .with_tag("limited"),
    )?;
    general.add_offer(
        ShopOffer::new(
            "flash_potion",
            "限时药水",
            Cost::new().currency("gold", 30),
            Reward::new().item("potion", 2),
        )
        .with_description("24 小时内有效。")
        .with_window(Some(now), Some(now + DAY_MS))
        .with_per_account_limit(2)
        .with_stock(10)
        .with_tag("flash"),
    )?;
    game.register_shop(general)?;

    // ---- premium: region-targeted + shop window + skins -----------------
    let mut premium = Shop::new("premium", "星穹商店")
        .with_description("只在极光星港营业的外观商店。")
        .with_regions([RegionId::from(REGION_AURORA)])
        .with_window(Some(now), Some(now + 7 * DAY_MS));
    premium.add_offer(
        ShopOffer::new(
            "frame_gold_offer",
            "黄金边框",
            Cost::new().currency("gems", 5),
            Reward::new().skin("frame_gold").xp(20),
        )
        .with_per_account_limit(1),
    )?;
    premium.add_offer(
        ShopOffer::new(
            "frame_aurora_offer",
            "极光边框",
            Cost::new().currency("gems", 3),
            Reward::new().skin("frame_aurora").xp(10),
        )
        .with_per_account_limit(1),
    )?;
    premium.add_offer(
        ShopOffer::new(
            "char_ranger_offer",
            "星港游侠",
            Cost::new().currency("gems", 3),
            Reward::new().skin("char_ranger"),
        )
        .with_per_account_limit(1),
    )?;
    premium.add_offer(
        ShopOffer::new(
            "badge_founder_offer",
            "拓荒者徽章",
            Cost::new().currency("gems", 8).currency("gold", 300),
            Reward::new().skin("badge_founder").xp(30),
        )
        .with_per_account_limit(1),
    )?;
    premium.add_offer(
        ShopOffer::new(
            "weapon_arc_offer",
            "电弧长枪",
            Cost::new().currency("gems", 4),
            Reward::new().skin("weapon_arc"),
        )
        .with_per_account_limit(1),
    )?;
    game.register_shop(premium)?;

    // ---- event shop: time window + stock + event currency ---------------
    let mut event = Shop::new("starfall", "星陨商队")
        .with_description("只在星陨节期间停靠。")
        .with_window(Some(now), Some(now + DAY_MS));
    event.add_offer(
        ShopOffer::new(
            "starfall_bundle",
            "星陨礼包",
            Cost::new().currency("gold", 200).resource("energy", 20),
            Reward::new()
                .skin("bg_nebula")
                .currency("astral", 2)
                .item("potion", 3)
                .xp(20),
        )
        .with_description("星云背景 + 2 张星尘券 + 3 瓶药水。")
        .with_stock(3)
        .with_per_account_limit(1),
    )?;
    event.add_offer(
        ShopOffer::new(
            "astral_emote",
            "星尘挥手",
            Cost::new().currency("astral", 1),
            Reward::new().skin("emote_wave"),
        )
        .with_description("用活动货币兑换的表情。")
        .with_stock(5)
        .with_per_account_limit(1),
    )?;
    game.register_shop(event)?;

    Ok(())
}

fn register_progression(game: &Game, now: i64) -> taixu::Result<()> {
    // ---- leaderboards: high, low, ties, top-N, seasons ------------------
    game.register_leaderboard(
        Leaderboard::new("power", "星港战力榜", ScoreOrder::HigherIsBetter)
            .with_description("等级和经验的综合评分。")
            .with_max_entries(50),
    )?;
    game.register_leaderboard(
        Leaderboard::new("speed", "远征竞速榜", ScoreOrder::LowerIsBetter)
            .with_description("单次远征用时，越低越好。")
            .with_max_entries(20),
    )?;
    game.register_leaderboard(
        Leaderboard::new("fortune", "星运榜", ScoreOrder::HigherIsBetter)
            .with_description("向导占卜出的今日星运指数。")
            .with_max_entries(20),
    )?;

    // ---- achievements (hidden + all reward kinds) -----------------------
    game.register_achievement(
        AchievementDef::new(
            "first_expedition",
            "初出茅庐",
            "expeditions",
            1,
            Reward::new().currency("gold", 50).xp(10),
        )
        .with_description("完成第一次远征。"),
    )?;
    game.register_achievement(
        AchievementDef::new(
            "veteran",
            "资深探险家",
            "expeditions",
            10,
            Reward::new()
                .currency("gems", 5)
                .resource("energy", 20)
                .item("potion", 2)
                .skin("frame_aurora")
                .xp(50),
        )
        .with_description("完成十次远征。"),
    )?;
    game.register_achievement(
        AchievementDef::new(
            "trader",
            "初次交易",
            "trades",
            1,
            Reward::new().currency("gold", 100).xp(20),
        )
        .with_description("完成第一次玩家交易。"),
    )?;
    game.register_achievement(
        AchievementDef::new(
            "socialite",
            "社交达人",
            "messages",
            10,
            Reward::new().currency("gems", 2).xp(10),
        )
        .with_description("在任何频道发送 10 条消息。"),
    )?;
    game.register_achievement(
        AchievementDef::new(
            "boss_hunter",
            "裂隙猎手",
            "boss",
            3,
            Reward::new().item("rift_shard", 2).xp(50),
        )
        .with_description("击败三次裂隙首领。"),
    )?;
    game.register_achievement(
        AchievementDef::new(
            "hidden_whisper",
            "太虚低语",
            "secret_found",
            1,
            Reward::new()
                .skin("other_halo")
                .item("mythic_core", 1)
                .xp(100),
        )
        .with_description("听见太虚深处的声音。")
        .hidden(),
    )?;
    game.register_achievement(AchievementDef::new(
        "artisan",
        "星港工匠",
        "crafts",
        3,
        Reward::new().currency("gold", 80),
    ))?;
    game.register_achievement(AchievementDef::new(
        "merchant",
        "星际商人",
        "sales",
        5,
        Reward::new().currency("gold", 80),
    ))?;
    game.register_achievement(AchievementDef::new(
        "quartermaster",
        "后勤主管",
        "upgrades",
        2,
        Reward::new().currency("gems", 2),
    ))?;
    game.register_achievement(AchievementDef::new(
        "collector",
        "常客",
        "purchases",
        5,
        Reward::new().currency("gold", 80),
    ))?;

    // ---- tasks: every objective kind, prerequisite, level, region, time,
    //      limit, once/daily/weekly/cooldown, hidden ----------------------
    game.register_task(
        TaskDef::new("tut_expedition", "第一次远征")
            .with_description("前往深空，完成一次远征。")
            .with_category("tutorial")
            .with_objective(TaskObjective::metric("expeditions", 1))
            .with_reward(Reward::new().currency("gold", 80).xp(20)),
    )?;
    game.register_task(
        TaskDef::new("tut_supply", "筹备补给")
            .with_description("持有 2 瓶星尘药水，并完成首次远征成就。")
            .with_category("tutorial")
            .with_objective(TaskObjective::item("potion", 2))
            .with_objective(TaskObjective::achievement("first_expedition"))
            .with_prerequisite("tut_expedition")
            .with_reward(Reward::new().currency("gems", 2).item("ore", 2).xp(10)),
    )?;
    game.register_task(
        TaskDef::new("tut_fashion", "星港门面")
            .with_description("拥有并装备极光边框，领取星环头像。")
            .with_category("tutorial")
            .with_objective(TaskObjective::skin("frame_aurora"))
            .with_prerequisite("tut_supply")
            .with_reward(
                Reward::new()
                    .skin("avatar_halo")
                    .currency("gold", 60)
                    .xp(10),
            ),
    )?;
    game.register_task(
        TaskDef::new("daily_gather", "每日采集")
            .with_description("在极光星港完成 2 次采集（30 分钟内）。")
            .with_category("daily")
            .with_objective(TaskObjective::metric("gathers", 2))
            .with_reward(Reward::new().currency("gold", 120).resource("energy", 20))
            .with_repeat(TaskRepeat::daily())
            .with_time_limit_ms(30 * 60_000)
            .available_in([RegionId::from(REGION_AURORA)]),
    )?;
    game.register_task(
        TaskDef::new("weekly_trade", "每周交易")
            .with_description("完成一次玩家交易（需要 2 级）。")
            .with_category("weekly")
            .with_objective(TaskObjective::metric("trades", 1))
            .with_reward(Reward::new().currency("gems", 3).currency("gold", 200))
            .with_repeat(TaskRepeat::weekly())
            .with_min_level(2),
    )?;
    game.register_task(
        TaskDef::new("boss_rush", "裂隙速攻")
            .with_description("击败 1 次首领并持有 1 枚裂隙碎片（10 分钟内）。")
            .with_category("combat")
            .with_objective(TaskObjective::metric("boss", 1))
            .with_objective(TaskObjective::item("rift_shard", 1))
            .with_reward(
                Reward::new()
                    .item("rift_shard", 1)
                    .currency("gold", 150)
                    .xp(40),
            )
            .with_repeat(TaskRepeat::cooldown(5 * 60_000))
            .with_time_limit_ms(10 * 60_000)
            .with_min_level(2),
    )?;
    game.register_task(
        TaskDef::new("event_starfall", "星陨节远征")
            .with_description("星陨节期间在星云前哨完成一次远征。")
            .with_category("event")
            .with_objective(TaskObjective::metric("expeditions", 1))
            .with_reward(Reward::new().currency("astral", 5).skin("bg_nebula").xp(30))
            .with_window(Some(now), Some(now + 7 * DAY_MS))
            .available_in([RegionId::from(REGION_NEBULA)]),
    )?;
    game.register_task(
        TaskDef::new("level_gate", "三级裂隙")
            .with_description("达到 3 级后击败一次首领。")
            .with_category("advanced")
            .with_objective(TaskObjective::metric("boss", 1))
            .with_reward(Reward::new().currency("gems", 5).xp(100))
            .with_min_level(3),
    )?;
    game.register_task(
        TaskDef::new("secret_trail", "太虚轨迹")
            .with_description("完成一次太虚窥探。")
            .with_category("secret")
            .with_objective(TaskObjective::metric("secret_found", 1))
            .with_reward(
                Reward::new()
                    .currency("gold", 150)
                    .item("mythic_core", 1)
                    .xp(50),
            )
            .hidden(),
    )?;

    // ---- collection: items + skins + achievements, 4 milestones ---------
    game.register_collection(
        CollectionDef::new("starport_album", "星港图鉴")
            .with_description("收集星港的材料、外观与成就。")
            .with_entries([
                Collectible::Item("ore".into()),
                Collectible::Item("potion".into()),
                Collectible::Item("rift_shard".into()),
                Collectible::Item("relic".into()),
                Collectible::Skin("frame_aurora".into()),
                Collectible::Skin("badge_founder".into()),
                Collectible::Skin("bg_nebula".into()),
                Collectible::Achievement("first_expedition".into()),
                Collectible::Achievement("veteran".into()),
                Collectible::Achievement("trader".into()),
            ])
            .with_milestones([
                CollectionMilestone::new(3, Reward::new().currency("gold", 100)),
                CollectionMilestone::new(
                    6,
                    Reward::new().currency("gems", 3).item("potion", 3).xp(20),
                ),
                CollectionMilestone::new(9, Reward::new().skin("bg_nebula").currency("gems", 2)),
                CollectionMilestone::new(
                    10,
                    Reward::new().item("trophy", 1).currency("gems", 5).xp(100),
                ),
            ]),
    )?;

    Ok(())
}

fn register_social_world(game: &Game, now: i64) -> taixu::Result<()> {
    // ---- announcements: all five kinds, priority, windows, regions ------
    game.publish_announcement(
        AnnouncementDraft::new(
            "欢迎登舰",
            "欢迎来到太虚星港。完成新手任务链，领取你的第一枚星钻。",
        )
        .with_kind(AnnouncementKind::System)
        .with_priority(0),
    )?;
    game.publish_announcement(
        AnnouncementDraft::new(
            "新手任务链开放",
            "任务页现在提供一条完整的新手路径：远征、补给、外观。",
        )
        .with_kind(AnnouncementKind::News)
        .with_priority(5),
    )?;
    game.publish_announcement(
        AnnouncementDraft::new(
            "星陨节 · 星陨商队抵达",
            "星陨商队将在星港停靠 24 小时，星云背景与星尘券限时上架。",
        )
        .with_kind(AnnouncementKind::Event)
        .with_priority(20)
        .with_window(Some(now), Some(now + 7 * DAY_MS))
        .with_regions([RegionId::from(REGION_AURORA), RegionId::from(REGION_NEBULA)]),
    )?;
    game.publish_announcement(
        AnnouncementDraft::new(
            "星穹商店上新",
            "极光星港的星穹商店新增黄金边框、极光边框与电弧长枪。",
        )
        .with_kind(AnnouncementKind::Promotion)
        .with_priority(10)
        .with_regions([RegionId::from(REGION_AURORA)]),
    )?;
    game.publish_announcement(
        AnnouncementDraft::new("漂流禁区维护中", "漂流禁区已关闭，等待下一次星潮开启。")
            .with_kind(AnnouncementKind::Maintenance)
            .with_priority(30)
            .with_window(Some(now), Some(now + 7 * DAY_MS))
            .with_regions([RegionId::from(REGION_DRIFT)]),
    )?;

    // ---- tavern room + NPC placements -----------------------------------
    let system = game.account(&SYSTEM_ACCOUNT.into())?;
    let tavern = game.create_room(
        &system.id,
        TAVERN_ROOM_NAME,
        &REGION_AURORA.into(),
        20,
        BTreeMap::from([
            ("topic".to_string(), "NPC 酒馆 · 交易与情报".to_string()),
            ("ambient".to_string(), "星港的霓虹在窗外流动".to_string()),
        ]),
    )?;

    game.register_npc(
        NpcDef::new(
            "barkeep",
            "酒馆老板",
            "一位在星港开了三百年酒馆的老兵，消息灵通，喜欢交易。",
        )
        .in_room(tavern.id.clone())
        .with_greeting("欢迎光临星港酒馆。想听点什么？")
        .with_metadata("role", "merchant"),
    )?;
    game.register_npc(
        NpcDef::new(
            "guide",
            "星港向导",
            "负责迎接新人的向导。会发放补给，也会为你占卜星运。",
        )
        .in_region(REGION_AURORA)
        .with_greeting("需要补给就对她说「补给」，想看看星运就说「占卜」。")
        .with_metadata("role", "guide"),
    )?;
    game.register_npc(
        NpcDef::new(
            "oracle",
            "星云占卜师",
            "驻留在星云前哨的占卜师，能从星尘里读出模糊的答案。",
        )
        .in_region(REGION_NEBULA)
        .with_greeting("星云会回答愿意倾听的人。"),
    )?;

    Ok(())
}

/// Agents are runtime-only (not serializable); reattach them after load.
pub fn build_npc_runtime(game: &Game) -> NpcRuntime {
    let mut runtime = NpcRuntime::new();
    if game.npc(&"guide".into()).is_ok() {
        runtime.register("guide", Arc::new(StarportGuide::new()));
    }
    if game.npc(&"barkeep".into()).is_ok() {
        let agent = RuleBasedNpc::new("来一杯星尘气泡水？")
            .with_rule("你好", "欢迎来到星港酒馆，旅行者。")
            .with_rule("hello", "欢迎来到星港酒馆，旅行者。")
            .with_rule("药水", "药水？去杂货铺看看，或者找其他玩家交易。")
            .with_rule("任务", "任务板在左侧，先完成第一次远征吧。")
            .with_rule("交易", "交易前记得双方确认，第二个人确认时才会结算。");
        runtime.register("barkeep", Arc::new(agent));
    }
    if game.npc(&"oracle".into()).is_ok() {
        let agent = RuleBasedNpc::new("星云沉默了片刻。")
            .with_rule("星运", "星运在 50 上下浮动，去问极光星港的向导会更准。")
            .with_rule("命运", "命运是星尘的另一种写法。");
        runtime.register("oracle", Arc::new(agent));
    }
    runtime
}
