use ruma::RoomId;

use crate::Result;

type SearchPdusResult<'a> = Result<Option<(Box<dyn Iterator<Item = Vec<u8>> + 'a>, Vec<String>)>>;

pub trait Data: Send + Sync {
	fn index_pdu(&self, shortroomid: u64, pdu_id: &[u8], message_body: &str) -> Result<()>;

	fn search_pdus<'a>(&'a self, room_id: &RoomId, search_string: &str) -> SearchPdusResult<'a>;
}
