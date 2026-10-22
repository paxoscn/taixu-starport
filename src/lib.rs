//! Taixu Starport host library.
//!
//! The Axum binary is thin; these modules are reusable and testable without
//! starting a server.

pub mod api;
pub mod error;
pub mod gameplay;
pub mod npc_agent;
pub mod seed;
pub mod state;
pub mod store;
