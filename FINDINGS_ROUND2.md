# Tai Xu（太虚）第二轮检验报告

> 背景：第一轮 7 项缺陷已在 `5928a28` 修复；第二轮问题在 `511e5dd`（`fix: address second-round API and behavior gaps`）修复并完成验收。G5 按宿主产品语义延期到 0.2。
> 结论：**G1/G2/G3/G4/G6/G7/G8/G9 已修复；G5 已接受延期**。没有新的阻塞性 bug。
> 探针：`tests/second_round_probe.rs`（9 个用例，**全部通过**；G5 无失败探针）

```bash
cd taixu-demo

# 第一轮 + 第二轮 + demo 集成测试，全部绿色
cargo test

# 单独跑第二轮回归
cargo test --test second_round_probe -- --test-threads=1
```

## 第二轮修复验收（511e5dd）

```text
cd ../taixu && cargo test --all-targets
# 44 passed; 0 failed（12 unit + 9 backend + 8 round-1 regressions
#                    + 10 round-2 regressions + 5 tasks）

cd ../taixu && cargo clippy --all-targets -- -D warnings
# clean

cd ../taixu-demo && cargo test
# demo_smoke: 1 passed
# library_probe: 6 passed
# second_round_probe: 9 passed (0 ignored)
```

无头 Chromium 验收也通过：任务锁定原因、非好友 DM 发现、头像/装备双向同步、Closed 区域只读+拒绝新活动、交易时间序、无 JS 异常。

| # | 类型 | 严重度 | 问题 | 状态 |
| --- | --- | --- | --- | --- |
| G1 | 确认缺陷 | 中低 | 可重复任务冷却结束后，`TaskView` 仍展示上一轮进度/时间戳；冷却结束但当前不满足条件时状态显示 `Claimed` 而非 `Locked` | **已修复**：新周期清零；不满足条件返回 `Locked` |
| G2 | 客户端 API 缺口 | 中 | `TaskView` 缺少锁定原因（前置/区域/时间窗） | **已修复**：新增 `requires` / `regions` / `starts_at_ms` / `ends_at_ms` |
| G3 | 客户端 API 缺口 | 中低 | 没有按账号列出聊天频道的 API，非好友 DM 无法自动发现 | **已修复**：新增 `Game::channels_for(account)` + `ChannelSummary` |
| G4 | API 可用性 | 低 | `Game::trades_for` 按 `TradeId` 键序返回，不是创建时间 | **已修复**：newest-first |
| G5 | 生产化风险 | 中 | 购买记录/交易/聊天频道/公告已读无保留策略；无账号删除；快照持续膨胀 | **0.2 延期**（见下） |
| G6 | 文档/语义 | 中低 | `RegionStatus::Closed` 文档写“关闭所有新活动”，实际只拦截新进入与房间活动 | **已修复**：Closed 状态矩阵落地，并补充文档 |
| G7 | 持久化 | 低 | `GameState` 没有 schema version/迁移机制 | **已修复**：`version` + `GAME_STATE_VERSION` |
| G8 | 文档/易用性 | 低 | `read_state()` 读锁 guard 与写方法组合可死锁 | **已修复**：新增 `with_state`，文档警告 |
| G9 | 设计一致性 | 低 | `Avatar.frame/badge` 与 `OwnedSkins.equipped` 是两份状态，不会自动同步 | **已修复**：双向同步 |

> 以下各节保留原始问题描述作为记录；G1–G4、G6–G9 已在 `511e5dd` 修复，G5 已接受延期到 0.2。

---

## G1（中低，确认缺陷）重复任务视图保留了上一轮数据

> **状态：已修复（511e5dd）。**

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
cargo test --test second_round_probe -- --test-threads=1
```

修复前输出：

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

**修复后**

库返回新周期视图；`web/app.js::taskCard` 的展示层归一化已移除。

---

## G2（中，客户端 API 缺口）`TaskView` 不暴露锁定原因

> **状态：已修复（511e5dd）。**

**位置**：`src/task.rs::TaskView`

`TaskView` 已有 `min_level`，但没有：

- `requires: Vec<TaskId>`
- `regions: Option<BTreeSet<RegionId>>`
- `starts_at_ms` / `ends_at_ms`

客户端只能看到 `Locked`，无法解释是“缺前置任务 / 不在本区域 / 时间窗未到”。直接下发 `task_definitions()` 会泄露隐藏任务（`TaskDef.hidden`、隐藏任务 ID）。

**复现**

```bash
cargo test --test second_round_probe -- --test-threads=1
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

**修复后**：`TaskView` 直接携带这些字段；`src/gameplay.rs::task_hints()` 已删除，前端 `taskHintHtml()` 改用 `task.requires/regions/starts_at_ms/ends_at_ms`。

---

## G3（中低，客户端 API 缺口）没有按账号列出聊天频道

> **状态：已修复（511e5dd）。**

修复前：`ChatService::channels()` 存在，但只能通过 `Game::read_state().chat.channels()` 访问；`Game` 没有面向账号的频道列表。

```rust
// 修复前不存在；511e5dd 新增
pub fn channels_for(&self, account: &AccountId) -> Result<Vec<ChannelSummary>>;
```

**修复前影响**：demo 的聊天频道选择器只能从好友列表推导 DM；非好友 DM 无法自动发现。**修复后**：demo 改用 `channels_for`，非好友 DM 也会出现在选择器中。

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

> **状态：已修复（511e5dd）。**

**位置**：`src/trade.rs::TradeBook::for_account` 按 `BTreeMap<TradeId, Trade>` 的键序返回；`TradeId` 是 UUID，因此顺序与创建时间无关。

**复现**

```bash
cargo test --test second_round_probe -- --test-threads=1
```

修复前输出示例（`created_at_ms` 乱序）：

```text
[("trade_0bc4…", 1006), ("trade_2756…", 1004), ("trade_4efd…", 1002),
 ("trade_8cee…", 1007), ("trade_a3bb…", 1003), ...]
```

**修复前影响**：宿主的“最近交易”列表必须自己排序。**修复后**：`trades_for` 已 newest-first，demo 的宿主排序已移除。

**建议**：库内按 `created_at_ms` 降序（tie-break `TradeId`）返回，或提供 `recent_trades(account, limit)`；若保持键序，文档需明确返回顺序不保证时间顺序。

---

## G5（中，生产化风险）快照/状态保留无上限，且没有账号删除

> **状态：0.2 延期（已接受）。**

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

### G5 延期决定（已接受，0.2）

G5 涉及三类策略性变更，不适合在未与宿主产品语义对齐时直接落地：

- **保留策略**：购买记录、交易历史、聊天频道、公告已读保留多久，涉及审计、客服追溯、合规与存储成本；默认值可能造成静默数据丢失。
- **账号删除/匿名化**：需要决定硬删除还是匿名化、是否保留财务记录、如何处理进行中的交易/房间/私聊；应配套墓碑机制和跨模块清理，而不是只删 `accounts` 表项。
- **购买限购索引优化**：把 `ShopBook.purchases` 改为增量索引属于持久化结构变更，应配合 schema version 与迁移方案一起做。

0.2 建议范围：可配置 retention + `erase_account` 墓碑 + 迁移；并与第一轮遗留的**通用宿主事务 API**、**货币目录**同期设计。当前只有 NPC 回合有专用原子提交入口；宿主组合流程（远征/合成/出售/扩容）仍需 `action_lock` + 补偿。

---

## G6（中低，文档/行为不一致）`Closed` 区域未阻止“所有新活动”

> **状态：已修复（511e5dd）。**

**位置**：`src/region.rs` 的 `RegionStatus` 文档；`src/game.rs::check_region_activity` 只被 `create_room` / `join_room` 使用。

**文档**：

```rust
/// Region is closed to all new activity.
Closed,
```

**修复前实际**：

- 新账号归属 / 跨区迁入：被 `check_region_accepts` 拦截；
- 建房 / 进房：被 `check_region_activity` 拦截；
- 区域聊天、购买、交易、加好友、接任务等：**仍可进行**。

**修复后**：Closed 状态矩阵落地；新房间、玩家聊天、购买、交易、好友申请、接任务均返回 `RegionNotOpen`；只读查询、领奖、放弃交易/任务、资料/外观编辑仍允许。demo 的宿主自定义活动（远征/合成/出售/扩容）也加了 `ensure_region_active`。

**复现**

```bash
cargo test --test second_round_probe -- --test-threads=1
```

修复前输出：

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

> **状态：已修复（511e5dd）。**

修复前：库推荐直接序列化 `GameState` 持久化，但快照没有版本字段。**修复后**：`GameState.version` + `GAME_STATE_VERSION`（当前 v1），旧快照缺字段时按当前版本读取。

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
cargo test --test second_round_probe -- --test-threads=1
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

> **状态：已修复（511e5dd）。**

`Game::read_state()` 返回 `RwLockReadGuard`。宿主若在同一线程持有 guard 时调用任何需要写锁的方法（`wallet`、`task_views`、`record_event`、`purchase` 等），parking_lot 的读写锁不可重入，会阻塞/死锁。

**修复后**：新增 `Game::with_state(|state| ...)` 短查询入口并补充死锁警告；demo 已把 `read_state()` 全部替换为 `with_state()`。

**建议**：

- 在文档中明确“不要把 guard 跨越写方法”；
- 或提供只读快照/专用查询 API，减少暴露 guard；
- 可选：调试构建下记录持锁线程，便于发现重入。

---

## G9（低，设计一致性）头像框/徽章与“已装备皮肤”是两份状态

> **状态：已修复（511e5dd）。**

- `Game::equip_skin` 更新 `OwnedSkins.equipped`；
- `Game::update_avatar` 单独设置 `Account.avatar.frame/badge`。

二者不会自动同步：

- 可以装备一个 Frame，但 `avatar.frame` 仍为空；
- 可以把 `avatar.frame` 设成“已拥有但未装备”的 Frame；
- 卸下 Frame 不会清空 `avatar.frame`。

**修复后**：`equip_skin` / `unequip_skin` / `update_avatar` 双向同步；demo 中的手工同步已移除。

**建议**：明确语义二选一：

1. `avatar.frame/badge` 是唯一的展示状态，`equip_skin` 同时更新它；或
2. `avatar.frame/badge` 只是“头像配置”，与 equipped map 独立，文档写清二者区别。

---

## 修复状态

1. **G1**：已修复，第二轮 2 个探针转正；
2. **G2 / G3**：已修复，`TaskView` 字段与 `channels_for` 落地，demo 已移除静态提示表与好友推导；
3. **G4**：已修复，`trades_for` newest-first；
4. **G6**：已修复，Closed 状态矩阵落地，demo 的宿主自定义活动（远征/合成/出售/扩容）也加了 `ensure_region_active`；
5. **G7 / G8 / G9**：已修复（`GAME_STATE_VERSION`、`with_state`、头像/装备双向同步）；
6. **G5**：0.2 延期，范围见上。

## 验收结果

```bash
cargo test --test second_round_probe -- --test-threads=1
# 9 passed; 0 failed

cargo test --test library_probe -- --test-threads=1
# 6 passed; 0 failed

cargo test
# demo_smoke 1 + library_probe 6 + second_round_probe 9 = 16 passed

cargo clippy --all-targets -- -D warnings
# clean
```

无头 Chromium 验收：任务锁定原因、非好友 DM、头像/装备同步、Closed 区域只读+拒绝新活动、交易时间序，均通过；无 JS 异常。

> 本报告没有修改 `../taixu`；所有探针都在 demo 仓库内。
