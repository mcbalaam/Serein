use crate::design::LazyHover;
use crate::markdown::{FormatCache, Formatted, discord_url};
use client_core::State;
use egui::RichText;
use model::{Id, Message};
use std::{
	collections::{BTreeMap, BTreeSet},
	hash::{DefaultHasher, Hash, Hasher},
};
mod actions;
mod banner;
mod debug;
mod intro;
mod layout;
mod rows;
mod system;
mod thread;
mod view;

#[cfg(all(debug_assertions, feature = "demo"))]
pub use self::actions::debug_copy_feedback_check;
use self::actions::*;
use self::banner::*;
#[cfg(feature = "demo")]
pub use self::debug::debug_unread_navigation_check;
use self::intro::*;
use self::layout::*;
pub(crate) use self::layout::{
	MESSAGE_LINE, compact_author_width, compact_header, fill_header_line, mentions_viewer,
};
use self::rows::*;
use self::system::*;
pub(crate) use self::thread::thread_activity;
use self::thread::*;

#[derive(Clone, Copy)]
enum TargetReveal {
	StayIfVisible,
	Center,
}
const REVEAL_SCROLL_SECS: f32 = 0.3;
/// Smooth return to the live edge from the jump-to-present control.
const PRESENT_SCROLL_SECS: f32 = 0.32;
/// Screens of history a reader must leave behind before the control appears.
const PRESENT_CONTROL_SCREENS: f32 = 6.0;
#[derive(Clone, Copy)]
struct RevealScroll {
	target: Id,
	from: f32,
	elapsed: f32,
}
#[derive(Default)]
pub struct TimelineView {
	polls: crate::polls::Cards,
	pub(super) poll_action: Option<(Id, client_core::polls::Action)>,
	pub(super) forward_request: Option<Id>,
	pub(super) sticker_request: Option<Id>,
	pub(super) browse_sticker: Option<model::Sticker>,
	pub(super) component_viewing: Option<(Id, u64)>,
	pub(super) components: crate::components::Components,
	pub(super) component_action: Option<crate::components::Action>,
	/// A private command response the reader dismissed this frame.
	pub(super) dismiss_ephemeral: Option<Id>,
	pub(super) extension_actions: std::sync::Arc<Vec<crate::extensions_ui::MenuAction>>,
	pub(super) extension_request: Option<(crate::extensions_ui::MenuAction, String)>,
	pub(super) user_action: Option<crate::user_menu::Action>,
	pub(super) restore_pending: Option<String>,
	/// A failed own row the reader dismissed this frame.
	pub(super) dismiss_pending: Option<String>,
	pub(super) cancel_upload: bool,
	pending_heights: BTreeMap<String, f32>,
	pub(super) hide_media_links: bool,
	pub(super) instant_scrolling: bool,
	applied_hide_media_links: bool,
	pub(super) compact_messages: bool,
	applied_compact_messages: bool,
	pub(super) gif_favorite: Option<model::Gif>,
	pub(super) invite_requests: Vec<String>,
	pub(super) invite_action: Option<crate::invites::Action>,
	pub(super) edit_started: bool,
	pub(super) reply_started: bool,
	pub(super) quick_delete: Option<(Id, Id)>,
	pub(super) channel_reference: Option<Id>,
	pub(super) channel_reference_load: Option<Id>,
	pub(super) pending_channel_reference: Option<Id>,
	pub(super) reply_target: Option<Id>,
	pending_reveal: Option<TargetReveal>,
	reveal_scroll: Option<RevealScroll>,
	/// Animated return to the live edge: offset it started from and elapsed seconds.
	present_scroll: Option<(f32, f32)>,
	/// Where the jump-to-present control was painted this frame, for hit tests in tests.
	pub(super) present_control: Option<egui::Rect>,
	highlighted: Option<(Id, f64)>,
	target_browsing: bool,
	hold_read_ack: bool,
	initial_read_checked: bool,
	pub(super) unread_jump: bool,
	pub(super) load_newer: bool,
	/// The loaded window does not fill the view; fetch newer messages without leaving it.
	pub(super) fill_newer: bool,
	pub(super) mark_read: Option<Id>,
	pub(super) mark_unread: Option<Id>,
	pub(super) auto_read_attempt: Option<Id>,
	at_current_latest: bool,
	/// The unread banner was raised during this visit; it stays until the reader leaves.
	unread_session: bool,
	/// Newest live-edge message the reader had on screen at the bottom during this visit.
	seen_latest: Option<Id>,
	/// Channel left this frame and the message to acknowledge there.
	pub(super) leave_read: Option<(Id, Id)>,
	/// The reader dismissed the unread banner while this was the channel's latest message.
	unread_dismissed: Option<Option<Id>>,
	/// The unread banner asked to acknowledge this channel up to its latest message.
	pub(super) mark_channel_read: Option<Id>,
	pub(super) reaction_picker: Option<(Id, egui::Rect, egui::Id)>,
	pub(super) reaction: Option<(Id, Option<model::ReactionEmoji>)>,
	pub(super) reaction_users: Option<(Id, model::ReactionEmoji, bool)>,
	/// Hover-bar reactions, most used first; `None` uses the defaults.
	pub(super) quick_reactions: Option<[&'static str; 3]>,
	/// Explicit preset for the double-click gesture; `None` disables it.
	pub(super) double_click_reaction: Option<&'static str>,
	/// A quick reaction the reader added this frame, for the usage ranking.
	pub(super) quick_reaction_used: Option<&'static str>,
	/// Requested pin change: channel, message, pinned.
	pub(super) pin_request: Option<(Id, Id, bool)>,
	/// Requested new thread: parent channel and the message that starts it.
	pub(super) thread_request: Option<(Id, Id)>,
	/// Starter message of the first visible thread card whose latest message is unknown.
	pub(super) thread_preview_request: Option<Id>,
	/// Channel whose Threads dialog a system row asked to open.
	pub(super) threads_request: Option<Id>,
	/// The starter message shown as a pseudo-row at the top of an exhausted thread.
	starter_row: Option<Id>,
	/// Whether the direct-message introduction leads the rows of an exhausted conversation.
	intro_row: bool,
	suppressed_deleted_highlight: BTreeSet<Id>,
	pub(super) remove_preserved: Option<Id>,
	toolbar: Option<(Id, egui::Rect)>,
	/// The message row the pointer was over last pass; its author's name style shows.
	name_hover: Option<Id>,
	copy_feedback: Option<(egui::Id, f64)>,
	heights: BTreeMap<Id, (u64, f32)>,
	// Heights can remain resize estimates; only these bounded active-row IDs were
	// measured with the current dimensions and state revision.
	measured_rows: BTreeSet<Id>,
	#[cfg(test)]
	leading_rendered: usize,
	pub(super) reflow_frames: u64,
	pub(super) consecutive_reflows: u64,
	width: f32,
	rows: Vec<(Id, f32)>,
	revision: u64,
	layout_fingerprint: u64,
	channel: Option<Id>,
	anchor: Option<(Id, f32)>,
	scroll_offset: f32,
	following: bool,
	formatted: FormatCache,
	pending_formatted: FormatCache,
	// A fingerprint of the revealed content prevents a reload that resets model revisions from
	// revealing edits, without cloning payloads. Pruned with the active window: at most 500 records.
	revealed: BTreeMap<Id, Revealed>,
	pub(super) viewing: Option<(Id, Id)>,
	embed_viewing: Option<(Id, u64, model::Attachment)>,
	/// Fixture-only: viewer to open once its message has arrived in the timeline.
	pending_viewer: Option<(Id, Id)>,
	pub(super) download: crate::attachments::DownloadUi,
	pub(super) audio: crate::audio::AudioUi,
	pub(super) video: crate::video::VideoUi,
	pub(super) opening: Option<String>,
	pub(super) browser_opening: Option<String>,
	text_size: f32,
	font_revision: (usize, usize, u32),
	scale: f32,
	pub(super) load_older: bool,
	pub(super) latest: bool,
	pub(super) visible_authors: Vec<Id>,
	jump: bool,
	unread_boundary: Option<Id>,
}
struct Revealed {
	/// Fingerprint of the content, embeds and attachments that were revealed.
	content: u64,
	text: u32,
	media: bool,
}
impl Revealed {
	fn fingerprint(message: &Message) -> u64 {
		let mut hasher = DefaultHasher::new();
		message.content.hash(&mut hasher);
		message.embeds.hash(&mut hasher);
		message.attachments.hash(&mut hasher);
		hasher.finish()
	}
	fn new(message: &Message, text: u32, media: bool) -> Self {
		Self {
			content: Self::fingerprint(message),
			text,
			media,
		}
	}
	fn matches(&self, message: &Message) -> bool {
		self.content == Self::fingerprint(message)
	}
}
#[cfg(test)]
#[path = "pending_tests.rs"]
mod pending_tests;

#[cfg(test)]
mod tests {
	use super::*;

	mod helpers;
	mod messages;
	mod scroll;
	mod threads;
	mod unread;
}
