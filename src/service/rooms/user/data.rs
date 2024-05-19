use ruma::{OwnedRoomId, OwnedUserId, RoomId, UserId};

use crate::Result;

pub trait Data: Send + Sync {
	fn reset_notification_counts(&self, user_id: &UserId, room_id: &RoomId) -> Result<()>;

	fn notification_count(&self, user_id: &UserId, room_id: &RoomId) -> Result<u64>;

	fn highlight_count(&self, user_id: &UserId, room_id: &RoomId) -> Result<u64>;

	// Returns the count at which the last reset_notification_counts was called
	fn last_notification_read(&self, user_id: &UserId, room_id: &RoomId) -> Result<u64>;

	fn associate_token_shortstatehash(&self, room_id: &RoomId, token: u64, shortstatehash: u64) -> Result<()>;

	fn get_token_shortstatehash(&self, room_id: &RoomId, token: u64) -> Result<Option<u64>>;

	fn get_shared_rooms<'a>(
		&'a self, users: Vec<OwnedUserId>,
	) -> Result<Box<dyn Iterator<Item = Result<OwnedRoomId>> + 'a>>;
}
