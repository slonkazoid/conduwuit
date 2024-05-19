use std::sync::Arc;

use ruma::{
	api::{
		client::{
			error::ErrorKind,
			profile::{get_avatar_url, get_display_name, get_profile, set_avatar_url, set_display_name},
		},
		federation,
	},
	events::{room::member::RoomMemberEventContent, StateEventType, TimelineEventType},
	presence::PresenceState,
};
use serde_json::value::to_raw_value;

use crate::{
	service::{pdu::PduBuilder, user_is_local},
	services, Error, Result, Ruma,
};

/// # `PUT /_matrix/client/r0/profile/{userId}/displayname`
///
/// Updates the displayname.
///
/// - Also makes sure other users receive the update using presence EDUs
pub(crate) async fn set_displayname_route(
	body: Ruma<set_display_name::v3::Request>,
) -> Result<set_display_name::v3::Response> {
	let sender_user = body.sender_user.as_ref().expect("user is authenticated");

	services()
		.users
		.set_displayname(sender_user, body.displayname.clone())
		.await?;

	// Send a new membership event and presence update into all joined rooms
	let all_rooms_joined: Vec<_> = services()
		.rooms
		.state_cache
		.rooms_joined(sender_user)
		.filter_map(Result::ok)
		.map(|room_id| {
			Ok::<_, Error>((
				PduBuilder {
					event_type: TimelineEventType::RoomMember,
					content: to_raw_value(&RoomMemberEventContent {
						displayname: body.displayname.clone(),
						join_authorized_via_users_server: None,
						..serde_json::from_str(
							services()
								.rooms
								.state_accessor
								.room_state_get(&room_id, &StateEventType::RoomMember, sender_user.as_str())?
								.ok_or_else(|| {
									Error::bad_database("Tried to send displayname update for user not in the room.")
								})?
								.content
								.get(),
						)
						.map_err(|_| Error::bad_database("Database contains invalid PDU."))?
					})
					.expect("event is valid, we just created it"),
					unsigned: None,
					state_key: Some(sender_user.to_string()),
					redacts: None,
				},
				room_id,
			))
		})
		.filter_map(Result::ok)
		.collect();

	for (pdu_builder, room_id) in all_rooms_joined {
		let mutex_state = Arc::clone(
			services()
				.globals
				.roomid_mutex_state
				.write()
				.await
				.entry(room_id.clone())
				.or_default(),
		);
		let state_lock = mutex_state.lock().await;

		_ = services()
			.rooms
			.timeline
			.build_and_append_pdu(pdu_builder, sender_user, &room_id, &state_lock)
			.await;
	}

	if services().globals.allow_local_presence() {
		// Presence update
		services()
			.presence
			.ping_presence(sender_user, &PresenceState::Online)?;
	}

	Ok(set_display_name::v3::Response {})
}

/// # `GET /_matrix/client/v3/profile/{userId}/displayname`
///
/// Returns the displayname of the user.
///
/// - If user is on another server and we do not have a local copy already
/// fetch displayname over federation
pub(crate) async fn get_displayname_route(
	body: Ruma<get_display_name::v3::Request>,
) -> Result<get_display_name::v3::Response> {
	if !user_is_local(&body.user_id) {
		// Create and update our local copy of the user
		if let Ok(response) = services()
			.sending
			.send_federation_request(
				body.user_id.server_name(),
				federation::query::get_profile_information::v1::Request {
					user_id: body.user_id.clone(),
					field: None, // we want the full user's profile to update locally too
				},
			)
			.await
		{
			if !services().users.exists(&body.user_id)? {
				services().users.create(&body.user_id, None)?;
			}

			services()
				.users
				.set_displayname(&body.user_id, response.displayname.clone())
				.await?;
			services()
				.users
				.set_avatar_url(&body.user_id, response.avatar_url.clone())
				.await?;
			services()
				.users
				.set_blurhash(&body.user_id, response.blurhash.clone())
				.await?;

			return Ok(get_display_name::v3::Response {
				displayname: response.displayname,
			});
		}
	}

	if !services().users.exists(&body.user_id)? {
		// Return 404 if this user doesn't exist and we couldn't fetch it over
		// federation
		return Err(Error::BadRequest(ErrorKind::NotFound, "Profile was not found."));
	}

	Ok(get_display_name::v3::Response {
		displayname: services().users.displayname(&body.user_id)?,
	})
}

/// # `PUT /_matrix/client/v3/profile/{userId}/avatar_url`
///
/// Updates the `avatar_url` and `blurhash`.
///
/// - Also makes sure other users receive the update using presence EDUs
pub(crate) async fn set_avatar_url_route(
	body: Ruma<set_avatar_url::v3::Request>,
) -> Result<set_avatar_url::v3::Response> {
	let sender_user = body.sender_user.as_ref().expect("user is authenticated");

	services()
		.users
		.set_avatar_url(sender_user, body.avatar_url.clone())
		.await?;

	services()
		.users
		.set_blurhash(sender_user, body.blurhash.clone())
		.await?;

	// Send a new membership event and presence update into all joined rooms
	let all_joined_rooms: Vec<_> = services()
		.rooms
		.state_cache
		.rooms_joined(sender_user)
		.filter_map(Result::ok)
		.map(|room_id| {
			Ok::<_, Error>((
				PduBuilder {
					event_type: TimelineEventType::RoomMember,
					content: to_raw_value(&RoomMemberEventContent {
						avatar_url: body.avatar_url.clone(),
						join_authorized_via_users_server: None,
						..serde_json::from_str(
							services()
								.rooms
								.state_accessor
								.room_state_get(&room_id, &StateEventType::RoomMember, sender_user.as_str())?
								.ok_or_else(|| {
									Error::bad_database("Tried to send displayname update for user not in the room.")
								})?
								.content
								.get(),
						)
						.map_err(|_| Error::bad_database("Database contains invalid PDU."))?
					})
					.expect("event is valid, we just created it"),
					unsigned: None,
					state_key: Some(sender_user.to_string()),
					redacts: None,
				},
				room_id,
			))
		})
		.filter_map(Result::ok)
		.collect();

	for (pdu_builder, room_id) in all_joined_rooms {
		let mutex_state = Arc::clone(
			services()
				.globals
				.roomid_mutex_state
				.write()
				.await
				.entry(room_id.clone())
				.or_default(),
		);
		let state_lock = mutex_state.lock().await;

		_ = services()
			.rooms
			.timeline
			.build_and_append_pdu(pdu_builder, sender_user, &room_id, &state_lock)
			.await;
	}

	if services().globals.allow_local_presence() {
		// Presence update
		services()
			.presence
			.ping_presence(sender_user, &PresenceState::Online)?;
	}

	Ok(set_avatar_url::v3::Response {})
}

/// # `GET /_matrix/client/v3/profile/{userId}/avatar_url`
///
/// Returns the `avatar_url` and `blurhash` of the user.
///
/// - If user is on another server and we do not have a local copy already
/// fetch `avatar_url` and blurhash over federation
pub(crate) async fn get_avatar_url_route(
	body: Ruma<get_avatar_url::v3::Request>,
) -> Result<get_avatar_url::v3::Response> {
	if !user_is_local(&body.user_id) {
		// Create and update our local copy of the user
		if let Ok(response) = services()
			.sending
			.send_federation_request(
				body.user_id.server_name(),
				federation::query::get_profile_information::v1::Request {
					user_id: body.user_id.clone(),
					field: None, // we want the full user's profile to update locally as well
				},
			)
			.await
		{
			if !services().users.exists(&body.user_id)? {
				services().users.create(&body.user_id, None)?;
			}

			services()
				.users
				.set_displayname(&body.user_id, response.displayname.clone())
				.await?;
			services()
				.users
				.set_avatar_url(&body.user_id, response.avatar_url.clone())
				.await?;
			services()
				.users
				.set_blurhash(&body.user_id, response.blurhash.clone())
				.await?;

			return Ok(get_avatar_url::v3::Response {
				avatar_url: response.avatar_url,
				blurhash: response.blurhash,
			});
		}
	}

	if !services().users.exists(&body.user_id)? {
		// Return 404 if this user doesn't exist and we couldn't fetch it over
		// federation
		return Err(Error::BadRequest(ErrorKind::NotFound, "Profile was not found."));
	}

	Ok(get_avatar_url::v3::Response {
		avatar_url: services().users.avatar_url(&body.user_id)?,
		blurhash: services().users.blurhash(&body.user_id)?,
	})
}

/// # `GET /_matrix/client/v3/profile/{userId}`
///
/// Returns the displayname, avatar_url and blurhash of the user.
///
/// - If user is on another server and we do not have a local copy already,
/// fetch profile over federation.
pub(crate) async fn get_profile_route(body: Ruma<get_profile::v3::Request>) -> Result<get_profile::v3::Response> {
	if !user_is_local(&body.user_id) {
		// Create and update our local copy of the user
		if let Ok(response) = services()
			.sending
			.send_federation_request(
				body.user_id.server_name(),
				federation::query::get_profile_information::v1::Request {
					user_id: body.user_id.clone(),
					field: None,
				},
			)
			.await
		{
			if !services().users.exists(&body.user_id)? {
				services().users.create(&body.user_id, None)?;
			}

			services()
				.users
				.set_displayname(&body.user_id, response.displayname.clone())
				.await?;
			services()
				.users
				.set_avatar_url(&body.user_id, response.avatar_url.clone())
				.await?;
			services()
				.users
				.set_blurhash(&body.user_id, response.blurhash.clone())
				.await?;

			return Ok(get_profile::v3::Response {
				displayname: response.displayname,
				avatar_url: response.avatar_url,
				blurhash: response.blurhash,
			});
		}
	}

	if !services().users.exists(&body.user_id)? {
		// Return 404 if this user doesn't exist and we couldn't fetch it over
		// federation
		return Err(Error::BadRequest(ErrorKind::NotFound, "Profile was not found."));
	}

	Ok(get_profile::v3::Response {
		avatar_url: services().users.avatar_url(&body.user_id)?,
		blurhash: services().users.blurhash(&body.user_id)?,
		displayname: services().users.displayname(&body.user_id)?,
	})
}
