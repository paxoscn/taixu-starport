# Tai Xu（太虚）库检验报告

> 检验对象：`../taixu`（`0.1.0`；初始 HEAD `957b402`，第一轮修复 `5928a28`，第二轮修复 `511e5dd`）
> 检验环境：Rust 1.90.0 / macOS
> 检验方式：通读全部模块 → 跑库自带测试/clippy → 针对高风险路径编写独立探针
> 探针：`tests/library_probe.rs`（第一轮 6/6 通过）、`tests/second_round_probe.rs`（第二轮 9/9 通过；G5 0.2 延期）

## 修复验收（2026-10-22）

库侧 `fix: address seven reported defects`（`5928a28`）已合入。重新验收结果：

```text
cargo test --test library_probe -- --test-threads=1
# 6 passed; 0 failed

cd ../taixu && cargo test --all-targets
# 34 passed; 0 failed (含 8 个新增回归测试 tests/regressions.rs)

cd ../taixu && cargo clippy --all-targets -- -D warnings
# clean
```

| # | 修复方式 | 回归验证 |
| --- | --- | --- |
| F1 | 新增 `Game::apply_npc_turn`，在单锁内预校验并原子提交动作+聊天 | `failed_npc_turn_leaves_no_rewards_scores_or_chat` |
| F2 | `ensure_account` 在写锁内完成检查+创建（`create_account_locked`） | `concurrent_ensure_account_resolves_to_one_account` |
| F3 | `ResourcePool.regen_remainder` 保存分数余量，读取即消费时间片 | `repeated_wallet_reads_do_not_duplicate_regeneration` |
| F4 | 物品/皮肤获取及奖励提交时刷新任务；过期前可记录完成 | `timed_item_task_completed_in_window_survives_expiry` |
| F5 | 新增 `check_region_activity`：Maintenance 只阻止新进入，已有玩家可继续房间活动 | `maintenance_allows_existing_players_to_use_rooms` |
| F6 | 新增 `validate_channel_targets`，运营消息校验区域/房间/私聊对象 | `system_messages_validate_channel_targets` |
| F7 | prelude 改导出 `GameResult`，不再遮蔽 `std::result::Result`；README 同步 | `prelude_result_does_not_conflict_with_trait_results` |

结论：**7 项全部已修复，探针全绿，可作为本 demo 的验收门槛。**

## 初始基线

库自带的质量基线是好的：

```text
cargo test --all-targets   # 修复前 24 passed
cargo clippy --all-targets -- -D warnings   # clean
```

模块划分清晰，核心原子操作（商店、交易、领奖）实现严谨，`GameError` 覆盖面广，快照/注入时钟的设计也适合宿主集成。

但按生产级开源库标准，发现了 **7 个需要修复的问题**。其中 2 个会造成经济/状态不一致，1 个会让并发登录偶发失败，1 个会丢失玩家已完成的限时任务，1 个是文档承诺与实现不一致，1 个是操作员 API 缺少校验，1 个是 README 示例无法编译。

复现命令：

```bash
cd taixu-demo
cargo test --test library_probe -- --test-threads=1
```

---

## F1（高）NPC 结构化动作并非原子执行，失败时会留下部分副作用

**位置**

- `src/npc.rs:371-451`，动作执行循环在 `414-430`
- `README.md:275-277`：“Agent 可以返回结构化动作，由运行时校验并原子执行”
- `src/npc.rs:8-15` 模块文档同样声称 action 会被应用；`NpcTurn.applied_actions` 暗示整轮成功

**现象**

`NpcRuntime::converse` 逐个调用 `game.grant_reward` / `game.set_leaderboard_score`。如果后面的动作失败（例如排行榜不存在、奖励物品不存在、背包已满），前面的动作已经提交，但函数返回 `Err`，且玩家消息/NPC 回复都不会写入聊天。

探针 `probe_npc_actions_are_atomic`：

```text
actions = [GiveReward(100 gold), SetScore(不存在的排行榜)]
结果：converse 返回 Err，但 Alice 的 gold = 100
```

**影响**

- LLM/规则 Agent 的 JSON 输出一旦部分非法，就会造成“奖励已发、对话丢失”的半完成状态。
- 可被用于刷奖励：让 Agent 先给出合法奖励，再附加一个必失败的动作。
- 与 README 的“原子执行”承诺直接冲突。

**建议修复**

1. 先做完整预校验（账号、排行榜、奖励引用的 item/skin/resource、文本长度、频道权限），全部通过后再统一提交；
2. 或给 `Game` 增加批量事务/回滚 API，由 `NpcRuntime` 在事务中执行；
3. 至少要在 `converse` 失败时回滚已执行的动作（快照相关账户资产并恢复）；
4. 增加回归测试：后续动作失败 → 无奖励、无聊天、无排行榜变化。

---

## F2（高）`ensure_account` 存在 TOCTOU 竞态，并发首登会随机失败

**位置**：`src/game.rs:287-298`

**现象**

```rust
if let Some(account) = self.state.read().accounts.get(&id) { return Ok(...); }
self.create_account(id, display_name, region)
```

读锁释放后才加写锁创建；两个请求同时进来时，都可能通过第一次检查，第二个 `create_account` 返回 `AccountAlreadyExists`。

探针 `probe_ensure_account_is_atomic`：16 线程同时 `ensure_account("alice", ...)`，稳定出现 2~4 次 `AccountAlreadyExists`。

**影响**

登录/自动注册接口在并发首登、重试、多实例转发时偶发 500，而且错误类型会误导调用方（账号其实已经存在）。

**建议修复**

- 在**同一把写锁内**完成“检查 + 插入”；或
- 捕获 `AccountAlreadyExists` 后重新读取并返回已有账号；
- 增加并发回归测试。

---

## F3（中高）资源再生可被“同一时间戳重复读取”刷高

**位置**：`src/wallet.rs:312-365`（`apply_regen`），关键在 `348-363`

**现象**

当 `regen_per_hour > 3_600_000`（即每秒可产 1 点以上）且两次读取间隔很短时：

```text
gained = elapsed_ms * rate / 3_600_000   // 可能是 1~N
consumed = gained * 3_600_000 / rate     // 向下取整后可能是 0
```

`consumed == 0` 时 `updated_at_ms` 不前进，但本次 `gained` 已经加到余额；同一时间戳再读一次，会把同一段 elapsed 再加一遍。

探针 `probe_resource_regen_repeated_reads`（`rate = 10_000_000/h`，时间戳都取 `t=1`）：

```text
第 1 次读 = 2，第 2 次读 = 4，第 3 次读 = 6
```

**影响**

宿主只要有一个轮询余额的接口（本项目的 Web 游戏就会轮询），高再生率资源就能无限增长。这是经济系统正确性问题，而不仅是精度问题。

**建议修复**

- 显式保存亚毫秒/分数余量（推荐 `remainder` 字段，或使用固定点）；
- 或当 `consumed == 0` 时不发放本次 `gained`；
- 属性测试：任意 rate、任意读取序列，余额不得超过 `cap`，且同一时刻重复读取不改变余额。

---

## F4（中）带时限任务会丢失“窗口内已完成”的道具/皮肤/成就目标

**位置**

- `src/game.rs:2543-2566`（`refresh_account_tasks` 跳过 `progress.is_expired(now_ms)`）
- `src/game.rs:1702-1729`（`claim_task` 只在领奖时刷新）
- `README.md:230-231`：“若在期限内完成，过期后仍可领奖”

**现象**

非 metric 目标（item/skin/achievement）不会在状态变化时自动打时间戳，只在 `task_view` / `record_event` / `claim_task` 时计算。如果玩家在窗口内满足条件，但在过期前没有任何查询，`claim_task` 的刷新会直接跳过已过期进度，`completed_at_ms` 永远为空，返回 `TaskNotCompleted`。

探针 `probe_expired_task_with_met_item_objective`：

```text
t=1000  接取 1s 限时任务（目标：持有 1 瓶药水）
t=1500  在窗口内获得药水（没有任何 task_view）
t=3000  claim_task -> Err(TaskNotCompleted)
```

**影响**

玩家按规则在时限内完成目标，却因为客户端没有恰好轮询而拿不到奖励；违反 README 的明确承诺。

**建议修复**

- 在 `add_item` / `grant_skin` / achievement 完成等状态变化时刷新相关任务（推荐）；
- 或为任务保存“目标满足时间”的证据；
- 或在过期后的首次刷新中，对仍然成立的目标允许标记完成（需明确语义）；
- 至少把文档改成“需在过期前查询过才会记录完成”，否则客户端无法可靠实现。

---

## F5（中）维护中的区域把“已有玩家”也冻结了，与文档不符

**位置**

- `src/region.rs:20-24`：`Maintenance` 文档写“Existing players may stay, but nobody new can enter”
- `src/game.rs:2101-2114`：`check_region_accepts` 对任何 `status != Open` 直接返回 `RegionNotOpen`
- 被 `create_room`（`game.rs:895`）和 `join_room`（`game.rs:948`）复用

**现象**

已有玩家所在区域切到 `Maintenance` 后：

```text
create_room -> RegionNotOpen
join_room   -> RegionNotOpen
```

玩家无法继续房间玩法，尽管他“留在”该区域。探针 `probe_maintenance_region_existing_player_activity` 覆盖了该行为。

**影响**

维护模式的实际语义是“冻结新进入 + 冻结房间活动”，但文档只承诺前者。运营切维护后，在线玩家会立即无法组队/进房。

**建议修复**

- 明确语义并二选一：
  1. 维护只阻止**新账号归属/跨区迁入**，本区已有账号仍可建房/进房；或
  2. 文档改为“维护期间禁止一切新的房间活动”。
- 给两种语义各写测试。

---

## F6（中低）`send_system_message` 不校验频道是否存在，会写入孤儿聊天记录

**位置**：`src/game.rs:1049-1061`

**现象**

与 `send_message` / `send_npc_message` 不同，这个运营接口不检查 `Room` / `Region` 是否存在：

```rust
game.send_system_message(ChatChannel::Room("room_missing".into()), "hello?") // Ok
```

探针 `probe_system_message_validates_channel` 复现。状态里会多出一个永远不可达的频道日志。

**影响**

运营脚本/后台配置写错 ID 时不报错，消息静默丢失；快照被污染，且没有清理接口。

**建议修复**

- 对 `Region` / `Room` 频道校验存在性（`Global` 除外）；
- 可选：对 `Private` 频道校验双方账号存在，或明确禁止系统私聊；
- 增加测试。

---

## F7（中）README 的 NPC Agent 示例无法编译

**位置**

- `README.md:238-256`
- 根因：`src/lib.rs:99,115` 的 `Result` 别名是 `Result<T, GameError>`，被 `prelude::*` 导出
- 示例中实现 `NpcAgent::respond` 时写的是 `-> Result<NpcDecision, NpcError>`

**现象**

按 README 原样抄写会得到：

```text
error[E0107]: type alias takes 1 generic argument but 2 generic arguments were supplied
error[E0053]: method `respond` has an incompatible type for trait
```

本报告的探针最初就是因此无法编译，改用 `std::result::Result<NpcDecision, NpcError>` 后才进入运行期。

**建议修复**

- 示例改为 `-> std::result::Result<NpcDecision, NpcError>`；或
- 不在 prelude 导出名为 `Result` 的别名（改为 `GameResult`），避免与标准库同名；
- 把 README 代码块纳入 doctest/编译测试，避免文档腐烂。

---

## 其他生产化建议（未单独写成失败探针）

这些不是当前 demo 的阻塞点，但属于生产级库应补齐的能力/说明：

1. **货币没有定义目录**：`CurrencyId` 是自由字符串，`GameState` 没有 `CurrencyCatalog`，README 却把“多货币”作为能力。客户端无法获取货币名称、图标、上限、是否付费货币；宿主只能硬编码。建议增加 `CurrencyDef` 注册表，或在文档中明确货币元数据由宿主维护。
2. **宿主组合流程没有事务 API**：库声称“所有跨模块操作都在一把写锁内完成”，但这只对库内置的固定流程（购买/交易/领奖）成立。宿主常见的“扣体力 → 发奖励”、“消耗道具 → 给货币”由多次公开调用组合，期间可被其他请求插入，失败时也无法回滚。建议提供 `Game::transact(|tx| ...)` 或在文档中明确“组合流程需宿主自行串行化并补偿”。本 demo 会用宿主侧互斥锁规避。
3. **注册时不做引用校验**：商店奖励/价格、任务目标、图鉴条目、成就奖励、任务前置、NPC 房间等都可以引用不存在的 ID。错误会延迟到购买/领奖/对话时暴露。建议注册时校验（或提供 `validate_world()`）。
4. **隐藏任务/隐藏成就的发现机制**：`task_views` 会过滤没有进度的隐藏任务，`accept_task` 又必须知道任务 ID；库内没有“由事件触发接取/揭示”的路径。若这是留给宿主的，建议在文档中写清；否则需要一个 trigger API。
5. **公告已读接口缺少可见性校验**：`mark_announcement_read` 只检查公告存在，不检查区域/时间窗；任意账号凭 ID 就能把看不到的公告标记为已读。建议校验 `targets_region` 与 `is_active`。
6. **`GameStore` 是同步 trait**：Postgres/Redis 实现会阻塞调用线程；文档应提示用 `spawn_blocking`，或提供 async store。
7. **少数校验不一致**：`Inventory::new(0)` 可构造零容量背包，`Inventory::with_contents` 可绕过堆叠上限；房间名/区域名/聊天正文不校验控制字符，而账号名/头像标题校验。建议统一校验策略。

---

## 第二轮检验（实现 demo 时发现）

> 完整可执行探针、API 建议与验收细节见 [FINDINGS_ROUND2.md](FINDINGS_ROUND2.md)。

第二轮共 9 项：G1（可重复任务视图保留上一轮数据，确认缺陷）、G2（TaskView 缺锁定原因）、G3（无按账号频道列表）、G4（trades_for 无时间序）、G5（保留策略/账号删除/限购索引）、G6（Closed 语义）、G7（快照 schema version）、G8（read_state guard 死锁风险）、G9（头像与装备皮肤两份状态）。

库侧 `511e5dd` 修复后：

- **G1–G4、G6–G9 已修复并验收通过**；`tests/second_round_probe.rs` 9/9 通过；库自身 44 项测试 + clippy 干净。
- demo 已移除以下 workaround：宿主 `task_hints()` 静态提示表、从好友列表推导 DM、宿主交易排序、`equip_skin`/`unequip_skin` 手工同步头像；改用 `TaskView.requires/regions/starts_at_ms/ends_at_ms`、`Game::channels_for`、库内 newest-first、双向同步、`with_state`。
- 宿主自定义活动（远征/合成/出售/扩容）新增 `ensure_region_active`，与库的 Closed 语义保持一致；无头浏览器验证了 Closed 区域只读 + 拒绝新活动、非好友 DM、头像同步、交易时间序。
- **G5 已接受延期到 0.2**：retention 涉及审计/客服/合规/存储成本；`erase_account` 需要墓碑与跨模块清理；购买限购索引属于持久化结构变更，应与 schema migration 一起做。0.2 建议范围：可配置 retention + `erase_account` 墓碑 + 迁移，并与第一轮遗留的**通用宿主事务 API**、**货币目录**同期设计。

第一轮的其他生产化建议仍未处理：注册时引用校验、隐藏任务/成就的触发路径、公告已读可见性、同步 `GameStore`、校验策略一致性。它们不是 demo 阻塞项，建议在 0.2 一并评估。

---

## 状态

- [x] 通读库与文档
- [x] 初始基线测试 + clippy
- [x] 编写 6 个探针复现第一轮 7 项问题
- [x] 第一轮库修复后探针 6/6 通过；库自身 34 项测试 + clippy 通过
- [x] 编写第二轮 9 个回归探针（G1–G4、G6–G9）
- [x] 第二轮库修复后探针 9/9 通过；库自身 44 项测试 + clippy 通过
- [x] 实现并验收 Web 游戏“太虚星港”（axum 后端 + 单页前端，覆盖全部模块）

Web 游戏见 [README.md](README.md)，第二轮细节见 [FINDINGS_ROUND2.md](FINDINGS_ROUND2.md)。G5（retention / erase_account / 限购索引）已接受 0.2 延期；第一轮的注册引用校验、隐藏任务触发、公告已读可见性、同步 `GameStore`、校验一致性仍未处理，建议与 G5、通用事务 API、货币目录一起纳入 0.2 评估。
