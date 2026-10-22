# Tai Xu（太虚）第二轮检验报告

> 背景：第一轮 7 项缺陷已在 `5928a28` 修复并通过验收。随后基于库实现 Web 游戏「太虚星港」时，又发现了若干问题。
> 结论：**没有新的阻塞性 bug**，游戏主流程可交付；但确认了 1 个视图缺陷，以及一组客户端 API、语义与生产化缺口。
> 探针：`tests/second_round_probe.rs`（6 个用例，默认 `#[ignore]`；当前**全部按预期失败**）

```bash
cd taixu-demo

# 第一轮验收 + demo 集成测试保持绿色
cargo test

# 第二轮探针（当前 6/6 失败，用于验收修复）
cargo test --test second_round_probe -- --ignored --test-threads=1
```

## 总览

| # | 类型 | 严重度 | 问题 | 探针 |
| --- | --- | --- | --- | --- |
| G1 | 确认缺陷 | 中低 | 可重复任务冷却结束后，`TaskView` 仍展示上一轮进度/时间戳；冷却结束但当前不满足条件时状态显示 `Claimed` 而非 `Locked` | 2 个 |
| G2 | 客户端 API 缺口 | 中 | `TaskView` 缺少锁定原因（前置/区域/时间窗） | 1 个 |
| G3 | 客户端 API 缺口 | 中低 | 没有按账号列出聊天频道的 API，非好友 DM 无法自动发现 | 无（API 不存在） |
| G4 | API 可用性 | 低 | `Game::trades_for` 按 `TradeId` 键序返回，不是创建时间 | 1 个 |
| G5 | 生产化风险 | 中 | 购买记录/交易/聊天频道/公告已读无保留策略；无账号删除；快照持续膨胀 | 无（规模问题） |
| G6 | 文档/语义 | 中低 | `RegionStatus::Closed` 文档写“关闭所有新活动”，实际只拦截新进入与房间活动 | 1 个 |
| G7 | 持久化 | 低 | `GameState` 没有 schema version/迁移机制 | 1 个 |
| G8 | 文档/易用性 | 低 | `read_state()` 读锁 guard 与写方法组合可死锁 | 无（死锁难安全测） |
| G9 | 设计一致性 | 低 | `Avatar.frame/badge` 与 `OwnedSkins.equipped` 是两份状态，不会自动同步 | 无（设计决策） |

---

## G1（中低，确认缺陷）重复任务视图保留了上一轮数据

**位置**

- `src/game.rs`：`task_view_in_state`、`task_status_in_state`
- `src/task.rs`：`TaskView` 定义

**现象**

```text
接取可重复任务 → 完成 → 领奖 → 冷却结束
task_view.status = Available
但 objectives[].progress 仍是上一轮的 1/1
accepted_at_ms / completed_at_ms / claimed_at_ms 仍是上一轮的值
```

如果冷却结束但当前因区域/等级/前置不满足而不可接取：

```text
task_view.status = Claimed
（而不是 Locked，客户端无法区分“冷却中”和“已冷却但条件不满足”）
```

**复现**

```bash
cargo test --test second_round_probe -- --ignored --test-threads=1 \
  probe_repeatable_task_available_after_cooldown_shows_fresh_progress \
  probe_repeatable_task_locked_after_cooldown_reports_locked
```

当前输出：

```text
left: 1, right: 0          # 上一轮 metric progress 泄漏到 Available 视图
left: Claimed, right: Locked  # 冷却已结束但区域不满足
```

**影响**

每日/每周/冷却任务在冷却结束后会显示“可接取 + 100% 完成”，玩家可能误以为可以立即再次领奖；前端无法仅凭 `TaskView` 区分真正可接取与条件不满足。

**建议修复**

- 当可重复任务进入新的可接取周期时，`TaskView` 使用空计数器/空时间戳；
- 冷却结束但当前条件不满足时，状态应为 `Locked`（或新增 `CooledDownButLocked`），并附带锁定原因（配合 G2）；
- 回归测试覆盖 metric 目标、item 目标、区域变化、冷却边界。

**demo workaround**

`web/app.js::taskCard` 对 `status=available && claimed_at_ms != null` 的任务把 metric 进度归零、隐藏旧时间戳，并显示“冷却已结束 · 接取新一轮会重置指标进度”。这只是展示层规避。

---

## G2（中，客户端 API 缺口）`TaskView` 不暴露锁定原因

**位置**：`src/task.rs::TaskView`

`TaskView` 已有 `min_level`，但没有：

- `requires: Vec<TaskId>`
- `regions: Option<BTreeSet<RegionId>>`
- `starts_at_ms` / `ends_at_ms`

客户端只能看到 `Locked`，无法解释是“缺前置任务 / 不在本区域 / 时间窗未到”。直接下发 `task_definitions()` 会泄露隐藏任务（`TaskDef.hidden`、隐藏任务 ID）。

**复现**

```bash
cargo test --test second_round_probe -- --ignored --test-threads=1 \
  probe_task_view_exposes_lock_reasons
```

**建议 API**

```rust
// 方案 A：把非敏感的锁定输入放进 TaskView
pub struct TaskView {
    // ...
    pub requires: Vec<TaskId>,
    pub regions: Option<BTreeSet<RegionId>>,
    pub starts_at_ms: Option<i64>,
    pub ends_at_ms: Option<i64>,
}

// 方案 B：显式计算锁定原因
pub enum TaskLockReason {
    Level { required: u32, current: u32 },
    Region { required: BTreeSet<RegionId>, current: RegionId },
    Prerequisite { missing: Vec<TaskId> },
    TimeWindow { starts_at_ms: Option<i64>, ends_at_ms: Option<i64> },
}
pub fn task_lock_reasons(&self, account: &AccountId, task: &TaskId) -> Result<Vec<TaskLockReason>>;
```

**demo workaround**：`src/gameplay.rs::task_hints()` 在宿主侧维护静态提示表；无法覆盖运营动态添加的任务。

---

## G3（中低，客户端 API 缺口）没有按账号列出聊天频道

`ChatService::channels()` 存在，但只能通过 `Game::read_state().chat.channels()` 访问；`Game` 没有面向账号的频道列表：

```rust
// 目前不存在
pub fn channels_for(&self, account: &AccountId) -> Vec<ChannelSummary>;
```

**影响**：demo 的聊天频道选择器只能从好友列表推导 DM；由交易邀请/运营系统消息创建的非好友 DM 无法自动发现，只能停留在当前频道回退显示。

**建议 API**

```rust
pub struct ChannelSummary {
    pub channel: ChatChannel,
    pub label: Option<String>,       // 房间名 / 对方显示名等由宿主补
    pub last_message_at_ms: Option<i64>,
    pub unread: usize,               // 若实现已读游标
}
pub fn channels_for(&self, account: &AccountId) -> Result<Vec<ChannelSummary>>;
```

或至少在文档中明确：宿主需通过 `read_state()` 自行推导，并说明 `channels()` 的生命周期/清理语义。

---

## G4（低，API 可用性）`trades_for` 不按时间排序

**位置**：`src/trade.rs::TradeBook::for_account` 按 `BTreeMap<TradeId, Trade>` 的键序返回；`TradeId` 是 UUID，因此顺序与创建时间无关。

**复现**

```bash
cargo test --test second_round_probe -- --ignored --test-threads=1 \
  probe_trades_for_returns_newest_first
```

当前输出示例（`created_at_ms` 乱序）：

```text
[("trade_0bc4…", 1006), ("trade_2756…", 1004), ("trade_4efd…", 1002),
 ("trade_8cee…", 1007), ("trade_a3bb…", 1003), ...]
```

**影响**：宿主的“最近交易”列表必须自己排序；如果忘记，最新交易可能不在最前。demo 在 `src/api.rs` 中按 `created_at_ms` 降序排序。

**建议**：库内按 `created_at_ms` 降序（tie-break `TradeId`）返回，或提供 `recent_trades(account, limit)`；若保持键序，文档需明确返回顺序不保证时间顺序。

---

## G5（中，生产化风险）快照/状态保留无上限，且没有账号删除

这些不是 demo 规模的问题，但长期运行会让 `GameState` 与快照持续膨胀：

| 数据 | 现状 | 风险 |
| --- | --- | --- |
| `ShopBook.purchases: Vec<PurchaseRecord>` | 只增不减；限购检查每次 O(n) | 购买记录无限增长，快照膨胀，购买变慢 |
| `TradeBook` | 保留全部 completed/cancelled/expired 交易 | 历史交易无限增长 |
| `ChatService.logs` | 每频道有界，但频道数无界（关闭房间、历史 DM） | 频道日志永不回收 |
| `AnnouncementBoard.reads` | 公告 → 账号集合；账号删除后不清理 | 已读集合膨胀 |
| 账号生命周期 | 没有 `Game::remove_account` / 匿名化 API；`FriendGraph::forget` 只清好友关系 | GDPR/账号注销无法完整实现；引用数据残留 |

**建议**：

- 提供保留策略/清理 API（按时间或条数裁剪购买、交易、聊天频道、已读）；
- 提供 `remove_account`（或 `erase_account`）及跨模块清理；
- 把购买限购计数改为按 `(shop, offer, account)` 的增量索引，避免 O(n) 扫描；
- 文档给出快照增长上界/定期压缩建议。

---

## G6（中低，文档/行为不一致）`Closed` 区域未阻止“所有新活动”

**位置**：`src/region.rs` 的 `RegionStatus` 文档；`src/game.rs::check_region_activity` 只被 `create_room` / `join_room` 使用。

**文档**：

```rust
/// Region is closed to all new activity.
Closed,
```

**实际**：

- 新账号归属 / 跨区迁入：被 `check_region_accepts` 拦截；
- 建房 / 进房：被 `check_region_activity` 拦截；
- 区域聊天、购买、交易、加好友、接任务等：**仍可进行**。

**复现**

```bash
cargo test --test second_round_probe -- --ignored --test-threads=1 \
  probe_closed_region_rejects_all_new_activity
```

当前输出：

```text
RegionStatus::Closed is documented as 'closed to all new activity',
but purchases still succeed
```

**建议**：二选一：

1. 把文档改为“Closed 阻止新进入与房间活动”，并明确其他模块不受影响；或
2. 为聊天/商店/交易/好友/任务等补齐 `Closed` 检查（并定义哪些只读操作仍允许）。

同时为 `Maintenance` / `Closed` 各自的模块行为写一张矩阵，避免宿主猜测。

---

## G7（低，持久化）`GameState` 没有 schema version/迁移机制

库推荐直接序列化 `GameState` 持久化，但快照没有版本字段：

```json
{
  "accounts": {},
  "achievements": {},
  "announcements": {},
  "chat": {},
  "collections": {},
  "cosmetics": {},
  "friends": {},
  "inventories": {},
  "items": {},
  "leaderboards": {},
  "npcs": {},
  "regions": {},
  "resources": {},
  "rooms": {},
  "shops": {},
  "skins": {},
  "tasks": {},
  "trades": {},
  "wallets": {}
}
```

**复现**

```bash
cargo test --test second_round_probe -- --ignored --test-threads=1 \
  probe_game_state_has_schema_version
```

**建议**：增加 `schema_version` 字段与迁移钩子，或提供带版本的 `SnapshotEnvelope`：

```rust
pub struct SnapshotEnvelope {
    pub version: u32,
    pub state: GameState,
}
pub fn migrate_snapshot(version: u32, value: serde_json::Value) -> Result<GameState>;
```

至少应在文档中说明“结构变更需宿主自行迁移”。

---

## G8（低，文档/易用性）`read_state()` 读锁 guard 的死锁风险

`Game::read_state()` 返回 `RwLockReadGuard`。宿主若在同一线程持有 guard 时调用任何需要写锁的方法（`wallet`、`task_views`、`record_event`、`purchase` 等），parking_lot 的读写锁不可重入，会阻塞/死锁。

demo 只在短作用域内使用 `read_state()`（如统计在线人数），但这是公开 API 的易踩坑点。

**建议**：

- 在文档中明确“不要把 guard 跨越写方法”；
- 或提供只读快照/专用查询 API，减少暴露 guard；
- 可选：调试构建下记录持锁线程，便于发现重入。

---

## G9（低，设计一致性）头像框/徽章与“已装备皮肤”是两份状态

- `Game::equip_skin` 更新 `OwnedSkins.equipped`；
- `Game::update_avatar` 单独设置 `Account.avatar.frame/badge`。

二者不会自动同步：

- 可以装备一个 Frame，但 `avatar.frame` 仍为空；
- 可以把 `avatar.frame` 设成“已拥有但未装备”的 Frame；
- 卸下 Frame 不会清空 `avatar.frame`。

demo 在 `src/api.rs` 的 `equip_skin` / `unequip_skin` 中手工同步 Frame/Badge。

**建议**：明确语义二选一：

1. `avatar.frame/badge` 是唯一的展示状态，`equip_skin` 同时更新它；或
2. `avatar.frame/badge` 只是“头像配置”，与 equipped map 独立，文档写清二者区别。

---

## 建议修复顺序

1. **G1**：确认缺陷，影响所有可重复任务的客户端展示，优先修；
2. **G2 / G3**：客户端 API 缺口，影响任何正式 UI 的任务解释与聊天发现；
3. **G6 / G4**：文档语义与返回顺序，修复成本低，能减少宿主猜测；
4. **G5 / G7**：生产规模与持久化，建议在 0.2 / 1.0 前补齐；
5. **G8 / G9**：文档与设计一致性，随文档更新处理。

## 验收方式

修复后：

1. 移除 `tests/second_round_probe.rs` 中对应用例的 `#[ignore]`；
2. `cargo test --test second_round_probe -- --test-threads=1` 全绿；
3. 第一轮探针 `cargo test --test library_probe -- --test-threads=1` 保持全绿；
4. demo `cargo test`、`cargo clippy --all-targets -- -D warnings` 保持绿色；
5. 如果 G2/G3 新增了 API，demo 将改用新 API 并移除 `task_hints()` / 好友推导等宿主 workaround。

> 本报告没有修改 `../taixu`；所有探针都在 demo 仓库内。
