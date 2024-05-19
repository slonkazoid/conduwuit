use ruma::{
	events::{receipt::ReceiptEvent, AnySyncEphemeralRoomEvent},
	serde::Raw,
	OwnedUserId, RoomId, UserId,
};

use crate::Result;

type AnySyncEphemeralRoomEventIter<'a> =
	Box<dyn Iterator<Item = Result<(OwnedUserId, u64, Raw<AnySyncEphemeralRoomEvent>)>> + 'a>;

pub trait Data: Send + Sync {
	/// Replaces the previous read receipt.
	fn readreceipt_update(&self, user_id: &UserId, room_id: &RoomId, event: ReceiptEvent) -> Result<()>;

	/// Returns an iterator over the most recent read_receipts in a room that
	/// happened after the event with id `since`.
	fn readreceipts_since(&self, room_id: &RoomId, since: u64) -> AnySyncEphemeralRoomEventIter<'_>;

	/// Sets a private read marker at `count`.
	fn private_read_set(&self, room_id: &RoomId, user_id: &UserId, count: u64) -> Result<()>;

	/// Returns the private read marker.
	///
	/// TODO: use this?
	#[allow(dead_code)]
	fn private_read_get(&self, room_id: &RoomId, user_id: &UserId) -> Result<Option<u64>>;

	/// Returns the count of the last typing update in this room.
	///
	/// TODO: use this?
	#[allow(dead_code)]
	fn last_privateread_update(&self, user_id: &UserId, room_id: &RoomId) -> Result<u64>;
}
