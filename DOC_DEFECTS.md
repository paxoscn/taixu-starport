# Tai Xu 文档缺陷报告（第三轮：文档可用性）

> 范围：基于 `../taixu` 实现「太虚星港」Web 游戏，以及两轮缺陷修复验收的过程中，哪些源码是我**被迫**阅读的。
> 结论：代码可用性已经通过两轮验收，但 **README + rustdoc 的组合不足以支撑宿主集成**。本文只提出**文档缺陷**，不要求改代码。
> 说明：第一轮/第二轮的代码缺陷见 [FINDINGS.md](FINDINGS.md) 与 [FINDINGS_ROUND2.md](FINDINGS_ROUND2.md)。

## 为什么需要读源码

README 是一份很好的 **feature tour + quick start**，但它不是 API reference。实现一个用到全部模块的宿主时，下面这些信息在 README 中找不到，只能读源码：

- 每个公开方法的完整签名、参数类型、返回类型、Builder 方法；
- 公开结构体/枚举的字段含义，以及 **serde wire format**；
- 每个操作可能返回哪些 `GameError`，失败时是否原子；
- 任务/交易/区域/房间/钱包等状态机的精确语义与时间边界；
- 哪些方法持写锁、哪些可以安全地在 `with_state` 内调用；
- NPC 的完整 prompt/action JSON 契约与原子回合规则；
- 快照版本、迁移、配置持久化、在线状态恢复等宿主职责；
- `GameConfig` 的完整字段、默认值与影响范围。

## 被迫阅读的源码清单

| 源码 | 为什么必须读 | 对应的文档缺口 |
| --- | --- | --- |
| `src/game.rs` | 所有门面方法签名、`GameConfig`、`GameState`、购买/交易/领奖原子流程、任务视图、NPC 回合、区域门禁、锁行为 | 没有逐方法 API 参考；没有锁/并发契约；没有错误/原子性矩阵 |
| `src/task.rs` | `TaskView` / `TaskObjective` / `TaskRepeat` / `TaskStatus` 的字段与 serde tag；任务注册校验 | 没有客户端 JSON 契约；没有任务状态机/时间边界文档 |
| `src/chat.rs` | `ChatChannel` 的 `Display`/serde 格式、私聊排序、`ChatAuthor` tag、消息字段、有界历史、`ChannelSummary` | 没有频道 wire format、访问规则、频道列表过滤/排序说明 |
| `src/item.rs` | `ItemDef` 字段、唯一/堆叠规则、槽位顺序、`try_add`/`try_remove` 全有或全无、`resize` 行为 | 没有背包槽位/叠堆/缩容的精确规则与示例 |
| `src/wallet.rs` | 货币/资源模型、cap、惰性再生公式、余量、原子扣费 | 没有再生公式、边界与快照影响说明 |
| `src/shop.rs` | `Shop`/`ShopOffer` 字段、库存、限购、时间窗、区域限制、`available_at` | 没有 Offer 字段参考与“是否可购买”判定说明 |
| `src/trade.rs` | `TradeState`、双方 side、`for_account` 顺序、资产不托管、结算校验 | 没有交易状态机、过期/取消/确认语义、列表顺序说明 |
| `src/cosmetics.rs` | `SkinCategory`（含 `other:name`）serde、`Avatar` 与 `OwnedSkins` 的权威语义 | 没有分类 wire format 与 Avatar/装备同步契约 |
| `src/collection.rs` | 图鉴条目/milestone 注册校验、实时进度语义、一次性领取 | 没有条目类型、milestone 约束、progress 字段说明 |
| `src/leaderboard.rs` | 并列名次、`top` 排序、`max_entries` 裁剪、赛季重置 | 没有排名/并列/Top-N 语义文档 |
| `src/achievement.rs` | 指标计数、隐藏成就过滤、claim 语义、`AchievementView` 字段 | 没有 Achievements 状态机/视图字段说明 |
| `src/announcement.rs` | `AnnouncementDraft`/`Announcement` 字段、优先级排序、区域/时间窗、已读 | 没有公告排序/已读/生命周期文档 |
| `src/npc.rs` | `NpcContext::system_prompt()` 的确切内容、`NpcAction`/`NpcDecision` JSON、`RuleBasedNpc`、运行时顺序 | 没有完整 NPC prompt/action schema 与集成指南 |
| `src/region.rs` | `RegionStatus` 语义、容量、玩家集合、endpoint | 修复前没有状态矩阵；现在 README 有矩阵，但逐方法门禁仍靠源码 |
| `src/room.rs` | 房主离开转移规则、成员/元数据字段、关闭/踢人行为 | 没有房间生命周期与所有权转移的精确说明 |
| `src/friend.rs` | 申请方向、`has_pending`、屏蔽时清理好友与申请、`forget` | 没有好友/屏蔽状态机与清理规则说明 |
| `src/store.rs` | `GameStore` 同步 trait、`load/save` 语义、`MemoryStore` | 没有持久化/迁移/宿主存储选型指南 |
| `src/clock.rs` | `ManualClock::advance` 返回值、`SystemClock` 边界 | 时间语义散落在 README，缺少全量说明 |
| `src/reward.rs` | `Reward`/`Cost` 字段、Builder、`scaled`（皮肤不重复） | 没有奖励/价格 bundle 的 JSON 契约与缩放规则 |
| `src/error.rs` | 全部 `GameError` 变体、字段 | 没有错误目录、HTTP 映射建议、逐方法错误列表 |
| `src/ids.rs` | 所有 ID newtype、serde transparent、map key 行为 | README 只列了部分 ID；没有序列化说明 |
| `src/account.rs` | `Account` 字段、显示名校验规则 | 没有账号字段/校验规则参考 |
| `tests/backend.rs`、`tests/tasks.rs`、`tests/regressions*.rs`、`examples/basic.rs` | 真实调用顺序与边界行为；事实上的“行为文档” | 测试没有作为文档入口；没有 cookbook/逐模块示例 |
| 两轮 `git diff`（`ade616a`/`511e5dd`） | 了解新增字段/API 与迁移点 | 没有 CHANGELOG、版本化 API 变更说明 |

## 文档缺陷清单

### D1（高）缺少逐模块 API reference

README 只有功能表和 quick start；没有每个模块的公开 API、字段、Builder 方法、示例。实现一个覆盖全部模块的宿主时，必须把 22 个 `src/*.rs` 当 API reference 读。

**建议**：发布 docs.rs；在 `docs/api/` 下按模块提供“签名 + 字段 + 可运行示例 + 常见错误”；或在 README 中维护 API 索引。

### D2（高）缺少宿主/客户端边界的 JSON 契约

前端/宿主必须知道以下 wire format，但文档没有集中给出：

- `ChatChannel` 的字符串格式（`global` / `region:<id>` / `room:<id>` / `dm:<len>:<a><b>`）；
- `ChatAuthor` 的 `{kind, account/npc}` tag；
- `TaskObjective` 的 `{type: metric|item|skin|achievement, ...}`；
- `TaskRepeat` 的 `{kind: once|cooldown, duration_ms}`；
- `TaskStatus`/`RegionStatus`/`TradeState` 的 snake_case；
- `SkinCategory` 的 `other:<name>`；
- `TaskView`/`AchievementView`/`ChannelSummary` 的字段；
- `GameState` 的 `version` 与顶层结构；
- `NpcDecision`/`NpcAction` 的 JSON。

**建议**：增加 `docs/json-contracts.md`，每个类型给出 JSON 示例，并把这些示例纳入 serde round-trip 测试；对客户端常用的 `TaskView`/`ChatMessage`/`NpcDecision` 额外给 TypeScript 类型示例。

### D3（高）缺少逐方法的错误/校验/原子性矩阵

`GameError` 有 60+ 变体，但文档没有说明每个方法可能返回哪些错误、失败时是否原子、哪些校验先于扣费。宿主必须读 `game.rs` + `error.rs` 才能把错误映射成 HTTP 状态码，并确认失败不会留下部分状态。

**建议**：为每个 `Game` 公开方法增加 rustdoc `# Errors` 段落；增加 `docs/errors.md`，包含：

- 错误变体目录与建议 HTTP status/code；
- 每个方法的错误列表；
- 原子性边界（“失败不会扣费/不会发奖/不会改库存”）；
- 典型错误恢复策略。

### D4（中高）缺少状态机与时间语义文档

以下状态机只能从源码还原：

- `TaskStatus`（Available/Locked/Active/Completed/Claimed/Expired）与 `TaskRepeat` 的冷却、新一轮、限时、过期后领奖；
- `TradeState` 的 Open/Completed/Cancelled/Expired 与 `expire_trades` 的宿主责任；
- `RegionStatus` 的 Open/Maintenance/Closed 对各模块的影响；
- 房主离开后的所有权转移；
- 钱包资源的惰性再生与 cap 边界；
- `online/last_seen` 的心跳/登出/重启语义。

**建议**：增加 `docs/state-machines.md`，用表格/状态图描述每个状态的进入/退出条件和时间边界；README 中的区域矩阵可以保留并链接到详细文档。

### D5（中高）缺少锁/并发契约

宿主很难从文档知道：

- `Game` 是同步 API，宿主需自行选择 async 运行时/线程；
- 所有状态在一把 `RwLock` 后；
- 哪些方法取写锁（`wallet`、`task_views`、`purchase`、`record_event` 等），哪些只读；
- `read_state()` 返回 guard 且不可重入，持 guard 调用写方法会死锁；
- `with_state` 可以安全用于短查询，但闭包内仍不能调用写方法。

第二轮的 G8 修复补充了 `with_state` 与死锁警告，但仍没有“方法 → 锁类型”的清单。

**建议**：在 `docs/hosting.md` 增加锁表（方法/读锁/写锁/可安全在 with_state 内调用）；rustdoc 中标注每个方法是否取写锁；给出 Axum/Tokio、Actix、同步线程池三种宿主示例。

### D6（中）缺少持久化/迁移指南

文档说明了 `GameState` 可序列化、`GameStore` 接口和 `GAME_STATE_VERSION`，但没有：

- 旧快照缺失 `version` 时的行为（当前按最新版本读取）；
- 版本不匹配时宿主应如何迁移；
- `GameConfig` 不随快照持久化，配置漂移的影响；
- 快照包含完整聊天/交易/购买历史，增长上界与压缩建议；
- `GameStore` 同步 trait 对 Postgres/Redis 的影响（需要 `spawn_blocking`）；
- 启动时 `online` 标记的恢复策略（demo 自行做了 mark-all-offline）；
- `MemoryStore` 与文件/对象存储的适用场景。

**建议**：增加 `docs/persistence.md` + 一个迁移示例（v1 → v2），并在 `GameState`/`GameStore` rustdoc 中链接。

### D7（中）缺少 NPC 集成契约

README 的 NPC 示例是高层描述，但没有：

- `NpcContext::system_prompt()` 的完整输出结构（人设、频道、附近玩家、历史、JSON 输出要求）；
- `NpcAction` 的完整 JSON、`Reward` 的嵌套结构；
- Agent 只能操作 speaker + nearby players 的规则；
- `apply_npc_turn` 的预校验/提交顺序、`applied_actions` 计数；
- Agent 运行时不属于 `GameState`、需在启动时重新注册；
- 失败时不写聊天、不发奖的原子保证（修复后已实现，但文档需要明确）。

**建议**：增加 `docs/npc.md`，包含完整 JSON schema、一个规则型 Agent、一个 LLM 调用示例（不绑定供应商）、以及 `converse` 的时序图。

### D8（中）缺少宿主职责/生命周期清单

宿主必须自己实现：

- 鉴权与会话（库不含）；
- 交易过期扫描（`expire_trades` 不会自动运行）；
- 在线状态清理（心跳/`last_seen`）；
- 快照频率与优雅退出保存；
- 账号删除/数据保留/审计（库目前没有 `remove_account`，G5 延期中）；
- 聊天/交易/购买历史的保留策略；
- 限流、内容审核、日志与监控。

README 零散提到一部分，但没有把这些作为“宿主契约”集中列出。

**建议**：增加 `docs/hosting.md` 的“Host responsibilities checklist”，并明确哪些是库保证、哪些必须宿主实现。

### D9（中低）`GameConfig` 缺少完整参考

README 只展示了 `default_inventory_capacity` 和 `trade_expiry_secs` 两个字段；实际有 12 个字段（聊天历史条数、房间上限、NPC 上下文条数、头像长度等）。宿主必须读 `game.rs` 才知道默认值与影响。

**建议**：`docs/config.md` 列出所有字段、默认值、影响模块、调优建议；在 `GameConfig` 每个字段上加 rustdoc。

### D10（中低）公开数据模型缺少逐字段 rustdoc

不少公开字段没有字段级说明，例：

- `ShopOffer.sold`（由后端维护，宿主不应手改）；
- `Region.players`（归属账号，不等于在线）；
- `Account.online/last_seen_ms`；
- `WalletSnapshot` 的货币/资源；
- `PurchaseReceipt` 的 paid/received；
- `ChannelSummary.message_count`；
- `TaskView.next_available_at_ms`；
- `Leaderboard.entries` 的公开 map；
- `ChatMessage.metadata` 的宿主自定义约定。

**建议**：对公共结构体字段补齐 rustdoc；开启 `#![warn(missing_docs)]`（或至少在 CI 中检查）；为容易误解的字段加“宿主不要修改/由库维护”的说明。

### D11（低）缺少版本化 API/CHANGELOG

第二轮 `511e5dd` 新增了 `TaskView.requires/regions/starts_at_ms/ends_at_ms`、`GameState.version`、`ChannelSummary`、`channels_for`、`with_state`，并改变了任务视图/Closed 语义。虽然都是增量/行为修正，但宿主需要知道兼容性与迁移点；目前只能读 `git diff`。

**建议**：增加 `CHANGELOG.md` + semver 策略；对行为修正（如任务新周期、Closed 矩阵）标注 “behavior change”；在 README 增加 “migrating from 0.1.x” 小节。

### D12（低）示例不足、测试未作为文档入口

`examples/basic.rs` 覆盖主流程，但以下场景没有可运行示例：任务四类目标/重复规则、商品时间窗/限购/区域限制、交易失败回滚、NPC 动作、区域状态矩阵、快照迁移、`with_state`、频道列表、头像/装备同步。实际行为只能从 `tests/backend.rs`、`tests/tasks.rs`、`tests/regressions*.rs` 中反推。

**建议**：为每个模块提供 `examples/*.rs` 或在 `docs/cookbook.md` 中按场景组织；把关键测试链接到文档（“行为由这些测试保证”）。

### D13（低）README 示例需要持续 doctest

F7 的 README NPC 示例曾因 `prelude::*` 导出 `Result<T, GameError>` 而无法编译。修复后示例可编译，但 README 代码块仍未全部纳入 doctest/CI。

**建议**：README 代码块使用 `no_run`/`ignore` 之外的 doctest 或单独编译测试；把 `cargo test --doc` 加入 CI。

## 已修复的文档相关问题（两轮）

| 项 | 原文档问题 | 当前状态 |
| --- | --- | --- |
| F7 | README NPC 示例因 prelude `Result` 无法编译 | 已修复（`GameResult`），建议加 doctest |
| F1 | NPC 动作“原子执行”与实现不符 | 已修复（`apply_npc_turn`），文档已更新 |
| F5 | Maintenance 语义与行为不符 | 已修复，文档更新 |
| G1 | 任务冷却后视图/状态未说明 | 已修复，README 补充新周期/`Locked` |
| G2 | `TaskView` 缺锁定原因 | 已修复，README 补充字段 |
| G3 | 无 `channels_for` 文档 | 已修复，README 功能表/代码已更新 |
| G4 | `trades_for` 顺序未说明 | 已修复，改为 newest-first |
| G6 | Closed 语义不清 | 已修复，README 增加状态矩阵 |
| G7 | 快照无版本说明 | 已修复，README 增加 `GAME_STATE_VERSION` |
| G8 | `read_state` 死锁风险未说明 | 已修复，README/rustdoc 增加 `with_state` 警告 |
| G9 | Avatar 与 equipped 语义不清 | 已修复，文档说明双向同步 |

这些例子说明：**把状态机/契约写进文档并用测试锁定**，比在 README 里只列 feature 更能减少宿主踩坑。

## 建议的文档结构

```text
README.md                    概览、安装、quick start、文档导航
docs/
  api/                       每个模块：签名、字段、示例、# Errors、锁类型
  json-contracts.md          客户端/宿主 JSON 契约 + serde round-trip 示例
  state-machines.md          任务/交易/区域/房间/在线/资源再生
  persistence.md             快照版本、迁移、GameStore、保留策略
  hosting.md                 鉴权边界、锁/并发、后台任务、宿主职责清单
  npc.md                     prompt/action schema、规则型/LLM 示例
  config.md                  GameConfig 全字段/默认值/影响
  errors.md                  GameError 目录 + HTTP 映射建议
  cookbook.md                按场景的可运行示例
CHANGELOG.md
```

## 文档验收标准

1. 每个公开方法/字段都有 rustdoc；公开枚举/结构体在 docs.rs 可检索；
2. 每个客户端/宿主边界的类型都有 JSON 示例，并纳入 serde round-trip 测试；
3. README 代码块全部通过 doctest/编译检查；
4. 每个模块至少一个可运行示例；
5. 状态机、区域矩阵、任务新周期、Closed 门禁都有表格或图；
6. 每个 `Game` 方法有 `# Errors` 与原子性说明；
7. `CHANGELOG.md` 记录行为修正与迁移点；
8. `cargo doc --no-deps` 在 CI 中无警告（或至少 `#![warn(missing_docs)]`）。

> 本报告没有修改 `../taixu`；它只记录实现 demo 时“必须读源码才能获得”的信息，作为文档改进清单。
