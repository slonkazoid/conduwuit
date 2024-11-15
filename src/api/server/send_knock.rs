use axum::extract::State;
use conduit::{err, pdu::gen_event_id_canonical_json, warn, Err, Error, PduEvent, Result};
use ruma::{
	api::{client::error::ErrorKind, federation::knock::send_knock},
	events::{
		room::member::{MembershipState, RoomMemberEventContent},
		StateEventType,
	},
	serde::JsonObject,
	OwnedServerName, OwnedUserId,
	RoomVersionId::*,
};

use crate::Ruma;

/// # `PUT /_matrix/federation/v1/send_knock/{roomId}/{eventId}`
///
/// Submits a signed knock event.
pub(crate) async fn create_knock_event_v1_route(
	State(services): State<crate::State>, body: Ruma<send_knock::v1::Request>,
) -> Result<send_knock::v1::Response> {
	if services
		.globals
		.config
		.forbidden_remote_server_names
		.contains(body.origin())
	{
		warn!(
			"Server {} tried knocking room ID {} who has a server name that is globally forbidden. Rejecting.",
			body.origin(),
			&body.room_id,
		);
		return Err!(Request(Forbidden("Server is banned on this homeserver.")));
	}

	if let Some(server) = body.room_id.server_name() {
		if services
			.globals
			.config
			.forbidden_remote_server_names
			.contains(&server.to_owned())
		{
			warn!(
				"Server {} tried knocking room ID {} which has a server name that is globally forbidden. Rejecting.",
				body.origin(),
				&body.room_id,
			);
			return Err!(Request(Forbidden("Server is banned on this homeserver.")));
		}
	}

	if !services.rooms.metadata.exists(&body.room_id).await {
		return Err(Error::BadRequest(ErrorKind::NotFound, "Room is unknown to this server."));
	}

	// ACL check origin server
	services
		.rooms
		.event_handler
		.acl_check(body.origin(), &body.room_id)
		.await?;

	let room_version_id = services.rooms.state.get_room_version(&body.room_id).await?;

	if matches!(room_version_id, V1 | V2 | V3 | V4 | V5 | V6) {
		return Err!(Request(Forbidden("Room version does not support knocking.")));
	}

	let Ok((event_id, value)) = gen_event_id_canonical_json(&body.pdu, &room_version_id) else {
		// Event could not be converted to canonical json
		return Err!(Request(InvalidParam("Could not convert event to canonical json.")));
	};

	let event_type: StateEventType = serde_json::from_value(
		value
			.get("type")
			.ok_or_else(|| Error::BadRequest(ErrorKind::InvalidParam, "Event missing type property."))?
			.clone()
			.into(),
	)
	.map_err(|_| Error::BadRequest(ErrorKind::InvalidParam, "Event has invalid event type."))?;

	if event_type != StateEventType::RoomMember {
		return Err(Error::BadRequest(
			ErrorKind::InvalidParam,
			"Not allowed to send non-membership state event to knock endpoint.",
		));
	}

	let content: RoomMemberEventContent = serde_json::from_value(
		value
			.get("content")
			.ok_or_else(|| Error::BadRequest(ErrorKind::InvalidParam, "Event missing content property"))?
			.clone()
			.into(),
	)
	.map_err(|_| Error::BadRequest(ErrorKind::InvalidParam, "Event content is empty or invalid"))?;

	if content.membership != MembershipState::Knock {
		return Err(Error::BadRequest(
			ErrorKind::InvalidParam,
			"Not allowed to send a non-knock membership event to knock endpoint.",
		));
	}

	// ACL check sender server name
	let sender: OwnedUserId = serde_json::from_value(
		value
			.get("sender")
			.ok_or_else(|| Error::BadRequest(ErrorKind::InvalidParam, "Event missing sender property."))?
			.clone()
			.into(),
	)
	.map_err(|_| Error::BadRequest(ErrorKind::BadJson, "sender is not a valid user ID."))?;

	services
		.rooms
		.event_handler
		.acl_check(sender.server_name(), &body.room_id)
		.await?;

	// check if origin server is trying to send for another server
	if sender.server_name() != body.origin() {
		return Err(Error::BadRequest(
			ErrorKind::InvalidParam,
			"Not allowed to knock on behalf of another server.",
		));
	}

	let state_key: OwnedUserId = serde_json::from_value(
		value
			.get("state_key")
			.ok_or_else(|| Error::BadRequest(ErrorKind::InvalidParam, "Event missing state_key property."))?
			.clone()
			.into(),
	)
	.map_err(|_| Error::BadRequest(ErrorKind::BadJson, "state_key is invalid or not a user ID."))?;

	if state_key != sender {
		return Err(Error::BadRequest(
			ErrorKind::InvalidParam,
			"State key does not match sender user",
		));
	};

	let origin: OwnedServerName = serde_json::from_value(
		serde_json::to_value(
			value
				.get("origin")
				.ok_or_else(|| Error::BadRequest(ErrorKind::InvalidParam, "Event missing origin property."))?,
		)
		.expect("CanonicalJson is valid json value"),
	)
	.map_err(|_| Error::BadRequest(ErrorKind::InvalidParam, "origin is not a server name."))?;

	let mut event: JsonObject = serde_json::from_str(body.pdu.get())
		.map_err(|_| Error::BadRequest(ErrorKind::InvalidParam, "Invalid knock event PDU."))?;

	event.insert("event_id".to_owned(), "$placeholder".into());

	let pdu: PduEvent = serde_json::from_value(event.into())
		.map_err(|_| Error::BadRequest(ErrorKind::InvalidParam, "Invalid knock event PDU."))?;

	let mutex_lock = services
		.rooms
		.event_handler
		.mutex_federation
		.lock(&body.room_id)
		.await;

	let pdu_id = services
		.rooms
		.event_handler
		.handle_incoming_pdu(&origin, &body.room_id, &event_id, value.clone(), true)
		.await?
		.ok_or_else(|| err!(Request(InvalidParam("Could not accept as timeline event."))))?;

	drop(mutex_lock);

	let knock_room_state = services.rooms.state.summary_stripped(&pdu).await;

	services
		.sending
		.send_pdu_room(&body.room_id, &pdu_id)
		.await?;

	Ok(send_knock::v1::Response {
		knock_room_state,
	})
}