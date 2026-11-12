use std::{fmt, future::Future};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthState {
	Unauthenticated,
	Authenticating,
	Challenged,
	Authenticated,
	Expired,
	Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
	#[error("Session expired; re-login manually")]
	Expired,
	#[error("Security challenge; unable to display captcha")]
	Challenged,
	#[error("Forbidden (403)")]
	Forbidden,
	#[error("Rate limited; wait before retrying")]
	RateLimited,
	#[error("Network connection failed")]
	Network,
	#[error("Something went wrong")]
	Ambiguous,
	#[error("Unknown server response")]
	Protocol,
	// Only compile-time labels are allowed here, never remote text, IDs or decode errors.
	#[error("{0}")]
	ProtocolAt(&'static str),
	#[error("Too much data received; wait before retrying")]
	Capacity,
	// Fixed local labels only; never include remote payloads or account identifiers.
	#[error("{0}")]
	CapacityAt(&'static str),
	#[error("Invalid session or bot account rejected")]
	InvalidCredential,
}
impl Failure {
	pub fn protocol_at(self, stage: &'static str) -> Self {
		if self == Self::Protocol {
			Self::ProtocolAt(stage)
		} else {
			self
		}
	}
	pub fn label(self) -> &'static str {
		match self {
			Self::Expired => "Session expired; re-login manually",
			Self::Challenged => "Security challenge; unable to display captcha",
			Self::Forbidden => "Forbidden (403)",
			Self::RateLimited => "Rate limited; wait before retrying",
			Self::Network => "Network connection failed",
			Self::Ambiguous => "Something went wrong",
			Self::Protocol => "Unknown server response",
			Self::ProtocolAt(stage) | Self::CapacityAt(stage) => stage,
			Self::Capacity => "Too much data received; wait before retrying",
			Self::InvalidCredential => "Invalid session or bot account rejected",
		}
	}
	pub fn ends_session(self) -> bool {
		matches!(
			self,
			Self::Expired
				| Self::Challenged
				| Self::Capacity
				| Self::CapacityAt(_)
				| Self::InvalidCredential
		)
	}
}
/// Intentionally neither Clone nor Serialize. Exposure is limited to transport adapters.
pub struct SessionSecret(Zeroizing<String>);
impl SessionSecret {
	pub fn from_owner_input(value: String) -> Result<Self, Failure> {
		let input = Zeroizing::new(value);
		// Browser storage shows the token JSON-quoted; pasting it verbatim would be rejected as expired.
		let trimmed = input.trim();
		let value = trimmed
			.strip_prefix('"')
			.and_then(|inner| inner.strip_suffix('"'))
			.unwrap_or(trimmed);
		if value.len() < 16
			|| value.len() > 2048
			|| value.starts_with("Bot ")
			|| value.starts_with("Bearer ")
			|| !value.bytes().all(|b| b.is_ascii_graphic())
		{
			return Err(Failure::InvalidCredential);
		}
		Ok(Self(Zeroizing::new(value.to_owned())))
	}
	pub fn expose(&self) -> &str {
		&self.0
	}
}
impl fmt::Debug for SessionSecret {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str("SessionSecret([REDACTED])")
	}
}
pub trait AuthProvider {
	fn authenticate(&mut self) -> impl Future<Output = Result<model::User, Failure>> + Send;
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn secret_is_redacted_and_header_injection_rejected() {
		let contextual = Failure::Protocol.protocol_at("Synthetic stage");
		assert_eq!(contextual.label(), "Synthetic stage");
		let capacity = Failure::CapacityAt("Synthetic capacity limit");
		assert_eq!(capacity.label(), "Synthetic capacity limit");
		assert!(capacity.ends_session());
		for failure in [
			Failure::Expired,
			Failure::Challenged,
			Failure::Forbidden,
			Failure::RateLimited,
			Failure::Network,
			Failure::Capacity,
			capacity,
			contextual,
		] {
			assert_eq!(failure.protocol_at("Other stage"), failure);
		}
		let secret = SessionSecret::from_owner_input("SYNTHETIC_SECRET_MARKER".into()).unwrap();
		assert!(!format!("{secret:?}").contains("SYNTHETIC"));
		for pasted in ["\"SYNTHETIC_SECRET_MARKER\"", " SYNTHETIC_SECRET_MARKER\n"] {
			let secret = SessionSecret::from_owner_input(pasted.into()).unwrap();
			assert_eq!(secret.expose(), "SYNTHETIC_SECRET_MARKER");
		}
		for value in [
			"Bot SYNTHETIC_SECRET",
			"Bearer SYNTHETIC_SECRET",
			"SYNTHETIC\r\nAuthorization: injected",
		] {
			assert!(SessionSecret::from_owner_input(value.into()).is_err());
		}
	}
}
