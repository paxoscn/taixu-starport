//! Shared server state, host-side sessions, and the authenticated extractor.
//!
//! Taixu explicitly leaves authentication to the host. This demo uses a
//! passwordless bearer token issued on login; it is deliberately simple and
//! clearly **not** production auth. The important boundary is that every
//! library call receives a trusted `AccountId` resolved from a session.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header, HeaderMap};
use parking_lot::Mutex;
use taixu::prelude::*;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::store::FileStore;

#[derive(Clone, Debug)]
pub struct Session {
    pub account: AccountId,
    pub last_seen_ms: i64,
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub account: AccountId,
    pub token: String,
}

pub struct AppState {
    pub game: Arc<Game>,
    pub store: Arc<FileStore>,
    pub npcs: Arc<NpcRuntime>,
    pub sessions: Mutex<HashMap<String, Session>>,
    /// Serializes host-composed flows (spend → grant → record). Taixu's own
    /// high-level operations are already atomic; this lock covers the glue.
    pub action_lock: Mutex<()>,
    pub admin_token: String,
    pub manual_clock: Option<Arc<ManualClock>>,
    pub started_at_ms: i64,
}

impl AppState {
    pub fn new(
        game: Arc<Game>,
        store: Arc<FileStore>,
        npcs: Arc<NpcRuntime>,
        admin_token: String,
        manual_clock: Option<Arc<ManualClock>>,
    ) -> Self {
        let started_at_ms = game.now_ms();
        Self {
            game,
            store,
            npcs,
            sessions: Mutex::new(HashMap::new()),
            action_lock: Mutex::new(()),
            admin_token,
            manual_clock,
            started_at_ms,
        }
    }

    pub fn now_ms(&self) -> i64 {
        self.game.now_ms()
    }

    pub fn create_session(&self, account: &AccountId) -> String {
        let token = Uuid::new_v4().simple().to_string();
        self.sessions.lock().insert(
            token.clone(),
            Session {
                account: account.clone(),
                last_seen_ms: self.game.now_ms(),
            },
        );
        token
    }

    pub fn remove_session(&self, token: &str) {
        self.sessions.lock().remove(token);
    }

    pub fn touch_session(&self, token: &str) {
        if let Some(session) = self.sessions.lock().get_mut(token) {
            session.last_seen_ms = self.game.now_ms();
        }
    }

    pub fn session_count(&self) -> usize {
        self.sessions.lock().len()
    }

    pub fn require_auth(&self, headers: &HeaderMap) -> ApiResult<AuthUser> {
        let raw = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ApiError::unauthorized("缺少 Authorization 头"))?;
        let token = raw
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| ApiError::unauthorized("Authorization 必须为 Bearer <token>"))?;
        let account = self
            .sessions
            .lock()
            .get(token)
            .map(|session| session.account.clone())
            .ok_or_else(|| ApiError::unauthorized("会话已失效，请重新登录"))?;
        Ok(AuthUser {
            account,
            token: token.to_owned(),
        })
    }

    pub fn is_admin(&self, headers: &HeaderMap) -> bool {
        headers
            .get("x-admin-token")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|token| token == self.admin_token)
    }

    pub fn require_admin(&self, headers: &HeaderMap) -> ApiResult<()> {
        if self.is_admin(headers) {
            Ok(())
        } else {
            Err(ApiError::forbidden("需要管理员令牌"))
        }
    }

    pub fn persist(&self) -> ApiResult<()> {
        self.game.save_to(&*self.store).map_err(ApiError::from)
    }
}

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> std::result::Result<Self, Self::Rejection> {
        state.require_auth(&parts.headers)
    }
}

pub fn demo_config() -> GameConfig {
    GameConfig {
        default_inventory_capacity: 40,
        max_inventory_capacity: 100,
        max_room_capacity: 50,
        max_name_len: 24,
        max_chat_message_len: 500,
        chat_history_per_channel: 150,
        trade_expiry_secs: 300,
        npc_context_messages: 20,
        npc_nearby_limit: 50,
        max_friend_request_len: 120,
        max_avatar_title_len: 32,
        max_avatar_image_len: 256,
    }
}

/// Snapshots must not claim anyone is online across a process restart.
pub fn mark_all_offline(game: &Game) {
    let ids: Vec<AccountId> = game.read_state().accounts.keys().cloned().collect();
    for id in ids {
        let _ = game.logout(&id);
    }
}
