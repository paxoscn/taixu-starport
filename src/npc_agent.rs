//! A small LLM-ready NPC agent that exercises all three structured actions.
//!
//! `NpcRuntime` owns the async turn; this agent only decides. It can be swapped
//! for an HTTP-backed model without touching the rest of the server: the
//! prompt is available from `NpcContext::system_prompt()` and the return type
//! is a serializable `NpcDecision`.

use std::collections::HashMap;

use async_trait::async_trait;
use parking_lot::Mutex;
use taixu::npc::{NpcAction, NpcAgent, NpcContext, NpcDecision, NpcError};
use taixu::prelude::*;

const SUPPLY_COOLDOWN_MS: i64 = 60_000;

pub struct StarportGuide {
    last_supply: Mutex<HashMap<AccountId, i64>>,
}

impl StarportGuide {
    pub fn new() -> Self {
        Self {
            last_supply: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for StarportGuide {
    fn default() -> Self {
        Self::new()
    }
}

fn fortune_score(account: &AccountId, now_ms: i64) -> i64 {
    let mut value = 0u64;
    for byte in account.as_str().bytes() {
        value = value.wrapping_mul(31).wrapping_add(u64::from(byte));
    }
    ((value ^ now_ms as u64) % 100) as i64
}

#[async_trait]
impl NpcAgent for StarportGuide {
    async fn respond(&self, ctx: &NpcContext) -> std::result::Result<NpcDecision, NpcError> {
        // The same string an LLM integration would send to a model. It
        // includes persona, channel, nearby players, recent history and the
        // required JSON action shape.
        let prompt = ctx.system_prompt();
        if std::env::var("TAIXU_DEBUG_PROMPT").is_ok() {
            eprintln!("--- NPC prompt for {} ---\n{prompt}", ctx.npc.name);
        }

        let message = ctx.message.to_lowercase();

        if message.contains("补给") || message.contains("supply") || message.contains("领取") {
            let mut last = self.last_supply.lock();
            if last
                .get(&ctx.speaker)
                .is_some_and(|at| ctx.now_ms.saturating_sub(*at) < SUPPLY_COOLDOWN_MS)
            {
                return Ok(NpcDecision::reply(
                    "补给每 60 秒只能领取一次。先去远征一趟吧。",
                ));
            }
            last.insert(ctx.speaker.clone(), ctx.now_ms);
            return Ok(NpcDecision {
                reply: Some("星港向导递给你一份应急补给。".into()),
                actions: vec![
                    NpcAction::Say {
                        text: "愿星光照亮你的航道。".into(),
                    },
                    NpcAction::GiveReward {
                        account: ctx.speaker.clone(),
                        reward: Reward::new()
                            .currency("gold", 30)
                            .resource("energy", 15)
                            .xp(5),
                    },
                ],
            });
        }

        if message.contains("占卜")
            || message.contains("星运")
            || message.contains("fortune")
            || message.contains("运气")
        {
            let score = fortune_score(&ctx.speaker, ctx.now_ms);
            return Ok(NpcDecision {
                reply: Some(format!(
                    "星尘排列成 {score}。数字越高，今天的裂隙越愿意让路。"
                )),
                actions: vec![NpcAction::SetScore {
                    leaderboard: "fortune".into(),
                    account: ctx.speaker.clone(),
                    score,
                }],
            });
        }

        if message.contains("秘密") || message.contains("太虚") || message.contains("secret") {
            return Ok(NpcDecision::reply(
                "有些秘密不会写在任务板上。去「太虚窥探」，留意突然出现的那条隐藏轨迹。",
            ));
        }

        if message.contains("任务") || message.contains("quest") || message.contains("帮助") {
            return Ok(NpcDecision::reply(
                "新手任务链在左侧任务页：先远征，再准备补给，最后买一个极光边框。\
                 我也可以给你补给（说「补给」）或占卜（说「占卜」）。",
            ));
        }

        Ok(NpcDecision::reply(
            "我是星港向导。你可以对我说「补给」「占卜」「任务」或「太虚秘密」。",
        ))
    }
}
