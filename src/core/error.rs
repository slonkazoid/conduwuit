use std::{convert::Infallible, fmt};

use axum::response::{IntoResponse, Response};
use bytes::BytesMut;
use http::StatusCode;
use http_body_util::Full;
use ruma::{
	api::{
		client::{
			error::{Error as RumaError, ErrorBody, ErrorKind},
			uiaa::{UiaaInfo, UiaaResponse},
		},
		OutgoingResponse,
	},
	OwnedServerName,
};
use thiserror::Error;
use tracing::error;
use ErrorKind::{
	Forbidden, GuestAccessForbidden, LimitExceeded, MissingToken, NotFound, ThreepidAuthFailed, ThreepidDenied,
	TooLarge, Unauthorized, Unknown, UnknownToken, Unrecognized, UserDeactivated, WrongRoomKeysVersion,
};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Error)]
pub enum Error {
	#[cfg(feature = "sqlite")]
	#[error("There was a problem with the connection to the sqlite database: {source}")]
	Sqlite {
		#[from]
		source: rusqlite::Error,
	},
	#[cfg(feature = "rocksdb")]
	#[error("There was a problem with the connection to the rocksdb database: {source}")]
	RocksDb {
		#[from]
		source: rust_rocksdb::Error,
	},
	#[error("Could not generate an image.")]
	Image {
		#[from]
		source: image::error::ImageError,
	},
	#[error("Could not connect to server: {source}")]
	Reqwest {
		#[from]
		source: reqwest::Error,
	},
	#[error("Could build regular expression: {source}")]
	Regex {
		#[from]
		source: regex::Error,
	},
	#[error("{0}")]
	Federation(OwnedServerName, RumaError),
	#[error("Could not do this io: {source}")]
	Io {
		#[from]
		source: std::io::Error,
	},
	#[error("There was a problem with your configuration file: {0}")]
	BadConfig(String),
	#[error("{0}")]
	BadServerResponse(&'static str),
	#[error("{0}")]
	/// Don't create this directly. Use Error::bad_database instead.
	BadDatabase(&'static str),
	#[error("uiaa")]
	Uiaa(UiaaInfo),
	#[error("{0}: {1}")]
	BadRequest(ErrorKind, &'static str),
	#[error("{0}")]
	Conflict(&'static str), // This is only needed for when a room alias already exists
	#[error("{0}")]
	Extension(#[from] axum::extract::rejection::ExtensionRejection),
	#[error("{0}")]
	Path(#[from] axum::extract::rejection::PathRejection),
	#[error("from {0}: {1}")]
	Redaction(OwnedServerName, ruma::canonical_json::RedactionError),
	#[error("{0} in {1}")]
	InconsistentRoomState(&'static str, ruma::OwnedRoomId),
	#[error("{0}")]
	AdminCommand(&'static str),
	#[error("{0}")]
	Err(String),
}

impl Error {
	pub fn bad_database(message: &'static str) -> Self {
		error!("BadDatabase: {}", message);
		Self::BadDatabase(message)
	}

	pub fn bad_config(message: &str) -> Self {
		error!("BadConfig: {}", message);
		Self::BadConfig(message.to_owned())
	}

	/// Returns the Matrix error code / error kind
	pub fn error_code(&self) -> ErrorKind {
		if let Self::Federation(_, error) = self {
			return error.error_kind().unwrap_or_else(|| &Unknown).clone();
		}

		match self {
			Self::BadRequest(kind, _) => kind.clone(),
			_ => Unknown,
		}
	}

	/// Sanitizes public-facing errors that can leak sensitive information.
	pub fn sanitized_error(&self) -> String {
		let db_error = String::from("Database or I/O error occurred.");

		match self {
			#[cfg(feature = "sqlite")]
			Self::Sqlite {
				..
			} => db_error,
			#[cfg(feature = "rocksdb")]
			Self::RocksDb {
				..
			} => db_error,
			Self::Io {
				..
			} => db_error,
			_ => self.to_string(),
		}
	}
}

impl From<Infallible> for Error {
	fn from(i: Infallible) -> Self { match i {} }
}

impl fmt::Debug for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self) }
}

#[derive(Clone)]
pub struct RumaResponse<T>(pub T);

impl<T> From<T> for RumaResponse<T> {
	fn from(t: T) -> Self { Self(t) }
}

impl From<Error> for RumaResponse<UiaaResponse> {
	fn from(t: Error) -> Self { t.to_response() }
}

impl Error {
	pub fn to_response(&self) -> RumaResponse<UiaaResponse> {
		if let Self::Uiaa(uiaainfo) = self {
			return RumaResponse(UiaaResponse::AuthResponse(uiaainfo.clone()));
		}

		if let Self::Federation(origin, error) = self {
			let mut error = error.clone();
			error.body = ErrorBody::Standard {
				kind: error.error_kind().unwrap_or_else(|| &Unknown).clone(),
				message: format!("Answer from {origin}: {error}"),
			};
			return RumaResponse(UiaaResponse::MatrixError(error));
		}

		let message = format!("{self}");
		let (kind, status_code) = match self {
			Self::BadRequest(kind, _) => (
				kind.clone(),
				match kind {
					WrongRoomKeysVersion {
						..
					}
					| Forbidden {
						..
					}
					| GuestAccessForbidden
					| ThreepidAuthFailed
					| UserDeactivated
					| ThreepidDenied => StatusCode::FORBIDDEN,
					Unauthorized
					| UnknownToken {
						..
					}
					| MissingToken => StatusCode::UNAUTHORIZED,
					NotFound | Unrecognized => StatusCode::NOT_FOUND,
					LimitExceeded {
						..
					} => StatusCode::TOO_MANY_REQUESTS,
					TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
					_ => StatusCode::BAD_REQUEST,
				},
			),
			Self::Conflict(_) => (Unknown, StatusCode::CONFLICT),
			_ => (Unknown, StatusCode::INTERNAL_SERVER_ERROR),
		};

		RumaResponse(UiaaResponse::MatrixError(RumaError {
			body: ErrorBody::Standard {
				kind,
				message,
			},
			status_code,
		}))
	}
}

impl ::axum::response::IntoResponse for Error {
	fn into_response(self) -> ::axum::response::Response { self.to_response().into_response() }
}

impl<T: OutgoingResponse> IntoResponse for RumaResponse<T> {
	fn into_response(self) -> Response {
		match self.0.try_into_http_response::<BytesMut>() {
			Ok(res) => res.map(BytesMut::freeze).map(Full::new).into_response(),
			Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
		}
	}
}
