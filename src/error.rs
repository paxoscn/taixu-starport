//! HTTP-facing error type. Maps `taixu::GameError` onto a small JSON envelope.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use taixu::store::StoreError;
use taixu::GameError;

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({ "error": self.message, "code": self.code })),
        )
            .into_response()
    }
}

impl From<GameError> for ApiError {
    fn from(error: GameError) -> Self {
        use GameError::*;
        let (status, code) = match &error {
            AccountNotFound(_)
            | RegionNotFound(_)
            | RoomNotFound(_)
            | ShopNotFound(_)
            | OfferNotFound(_)
            | TradeNotFound(_)
            | SkinNotFound(_)
            | CollectionNotFound(_)
            | MilestoneNotFound(_)
            | LeaderboardNotFound(_)
            | AchievementNotFound(_)
            | TaskNotFound(_)
            | AnnouncementNotFound(_)
            | NpcNotFound(_)
            | MessageNotFound(_)
            | UnknownItem(_)
            | UnknownResource(_)
            | FriendRequestNotFound => (StatusCode::NOT_FOUND, "not_found"),

            AlreadyExists(_)
            | AccountAlreadyExists(_)
            | RegionAlreadyExists(_)
            | ItemAlreadyExists(_)
            | ResourceAlreadyExists(_)
            | ShopAlreadyExists(_)
            | OfferAlreadyExists(_)
            | SkinAlreadyExists(_)
            | CollectionAlreadyExists(_)
            | LeaderboardAlreadyExists(_)
            | AchievementAlreadyExists(_)
            | TaskAlreadyExists(_)
            | NpcAlreadyExists(_)
            | AlreadyFriends(_)
            | FriendRequestPending
            | AlreadyBlocked
            | NotBlocked
            | AlreadyClaimed
            | AchievementAlreadyClaimed
            | TaskAlreadyAccepted(_)
            | TaskAlreadyCompleted(_)
            | TaskAlreadyClaimed(_)
            | TaskOnCooldown { .. }
            | TaskNotAccepted(_)
            | TaskNotCompleted(_)
            | TaskLocked(_)
            | AchievementNotCompleted
            | MilestoneLocked
            | OfferUnavailable
            | OutOfStock
            | PurchaseLimitReached
            | InsufficientFunds { .. }
            | InsufficientResources { .. }
            | InsufficientItems { .. }
            | InventoryFull
            | TradeExpired
            | TradeAssetsUnavailable
            | EmptyTrade => (StatusCode::CONFLICT, "conflict"),

            RegionNotOpen(_)
            | RegionFull(_)
            | RoomFull(_)
            | AlreadyInRoom { .. }
            | NotInRoom(_)
            | NotRoomMember(_, _)
            | NotRoomOwner
            | CannotFriendSelf
            | Blocked
            | CannotTradeWithSelf
            | NotTradeParticipant(_)
            | TradeNotOpen
            | ChannelAccessDenied(_)
            | SkinNotOwned(_)
            | SkinCategoryMismatch { .. } => (StatusCode::FORBIDDEN, "forbidden"),

            InvalidInput(_)
            | InvalidName(_)
            | InvalidQuantity(_)
            | InvalidRoomCapacity { .. }
            | InvalidAnnouncementWindow
            | EmptyContent
            | ContentTooLong { .. }
            | AmountOverflow => (StatusCode::BAD_REQUEST, "bad_request"),

            NpcDisabled(_) => (StatusCode::CONFLICT, "conflict"),
            Persistence(_) => (StatusCode::INTERNAL_SERVER_ERROR, "persistence_error"),
        };
        ApiError::new(status, code, error.to_string())
    }
}

impl From<StoreError> for ApiError {
    fn from(error: StoreError) -> Self {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "persistence_error",
            error.to_string(),
        )
    }
}

pub type ApiResult<T> = std::result::Result<T, ApiError>;
