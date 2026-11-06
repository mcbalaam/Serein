use super::*;

pub fn visible_range(rows: &[(Id, f32)], min: f32, max: f32) -> (usize, usize, f32) {
	let mut top = 0.0;
	let mut first = 0;
	while first < rows.len() && top + rows[first].1 <= min {
		top += rows[first].1;
		first += 1;
	}
	let mut end = first;
	let mut bottom = top;
	while end < rows.len() && bottom < max {
		bottom += rows[end].1;
		end += 1;
	}
	(first, end, top)
}
pub(super) fn ease_out_cubic(t: f32) -> f32 {
	let rest = 1.0 - t;
	1.0 - rest * rest * rest
}
pub(super) fn centered_offset(rows: &[(Id, f32)], id: Id, viewport_h: f32, packed: f32) -> f32 {
	let row_top = anchor_offset(rows, id, 0.0);
	let row_h = rows
		.iter()
		.find(|(row, _)| *row == id)
		.map_or(0.0, |(_, height)| *height);
	(row_top - (viewport_h - row_h.min(viewport_h)) * 0.5)
		.clamp(0.0, (packed - viewport_h).max(0.0))
}
pub(super) fn anchor_offset(rows: &[(Id, f32)], id: Id, inset: f32) -> f32 {
	if rows.is_empty() {
		return 0.0;
	}
	// Keep the next surviving message at the top; fall back to the previous one at the end.
	let index = rows
		.partition_point(|(row, _)| *row < id)
		.min(rows.len() - 1);
	let within = if rows[index].0 == id {
		// A short bottom-aligned page stores leading space as a negative inset.
		// Keep that space when older rows arrive; only the final scroll offset is nonnegative.
		inset.min(rows[index].1.max(0.0))
	} else {
		0.0
	};
	(rows[..index].iter().map(|(_, height)| *height).sum::<f32>() + within).max(0.0)
}
pub(super) fn layout_key(message: &Message) -> u64 {
	// A layout fingerprint only; spoiler visibility uses exact text instead.
	let mut key = DefaultHasher::new();
	message.content.hash(&mut key);
	for user in &message.mentions {
		user.id.hash(&mut key);
		user.name.hash(&mut key);
	}
	message.author.name.hash(&mut key);
	message.author.kind.hash(&mut key);
	message.author.webhook.hash(&mut key);
	message.edited.hash(&mut key);
	message.reply_to.hash(&mut key);
	if let Some(interaction) = &message.interaction {
		interaction.user.id.hash(&mut key);
		interaction.user.name.hash(&mut key);
		interaction.command.hash(&mut key);
	}
	message.forwarded.hash(&mut key);
	message.reply_deleted.hash(&mut key);
	message.unsupported.hash(&mut key);
	message.extra_content.hash(&mut key);
	message.poll.hash(&mut key);
	message.sticker_items.hash(&mut key);
	message.components.hash(&mut key);
	message.kind.hash(&mut key);
	message.attachments.hash(&mut key);
	message.embeds.hash(&mut key);
	message.embeds_suppressed.hash(&mut key);
	match message.reactions.as_deref() {
		Some(reactions) => {
			true.hash(&mut key);
			for reaction in reactions {
				reaction.emoji.hash(&mut key);
			}
		}
		None => false.hash(&mut key),
	}
	key.finish()
}
pub(crate) const MESSAGE_LINE: f32 = 22.0;
/// Widest author name in a compact row; the body takes the rest beside it.
pub(crate) fn compact_author_width(available: f32) -> f32 {
	(available * 0.35).clamp(72.0, 150.0)
}
/// Match the first body-text baseline, including custom fonts and reading text sizes.
pub(crate) fn compact_header(
	ui: &mut egui::Ui,
	width: f32,
	font: egui::FontId,
	add: impl FnOnce(&mut egui::Ui),
) {
	let body = ui.painter().layout_no_wrap(
		" ".into(),
		egui::TextStyle::Body.resolve(ui.style()),
		egui::Color32::TRANSPARENT,
	);
	let header = ui
		.painter()
		.layout_no_wrap(" ".into(), font, egui::Color32::TRANSPARENT);
	let offset = body.rows[0].glyphs[0].pos.y - header.rows[0].glyphs[0].pos.y;
	let at = ui.next_widget_position();
	let mut header_ui = ui.new_child(
		egui::UiBuilder::new()
			.max_rect(egui::Rect::from_min_size(
				at + egui::vec2(0.0, offset),
				egui::vec2(width, body.size().y),
			))
			.layout(egui::Layout::left_to_right(egui::Align::Min)),
	);
	add(&mut header_ui);
	let used = header_ui.min_rect();
	// Negative font offsets must not move the next column's origin above this row.
	ui.advance_cursor_after_rect(egui::Rect::from_min_size(
		at,
		egui::vec2(
			used.width(),
			body.size().y.max(used.height()).max(used.bottom() - at.y),
		),
	));
}
pub(super) const GROUPED_ROW_SAVINGS: f32 = 52.0;
/// Space above a new message group: cozy by default, tighter when compact spacing is on.
pub(crate) fn group_gap(compact_messages: bool) -> i8 {
	if compact_messages { 4 } else { 10 }
}
pub(super) fn reserved_chrome(ui: &egui::Ui, message: &Message, width: f32) -> f32 {
	let reactions = crate::reactions::estimated_height(
		ui,
		message.reactions.as_deref(),
		(width - 88.0).max(40.0),
	);
	let components = 40.0 * (message.components.len().min(5) as f32);
	let stickers = 160.0 * (message.sticker_items.len().min(4) as f32);
	reactions
		+ components
		+ stickers
		+ message
			.poll
			.as_ref()
			.map_or(0.0, |p| 140.0 + 64.0 * p.answers.len() as f32)
}
pub(crate) fn fill_header_line(ui: &mut egui::Ui, compact: bool, text_line: egui::Rect) {
	let slack = MESSAGE_LINE - text_line.height();
	if !compact && slack > 0.0 && ui.min_rect().bottom() - text_line.bottom() < 1.0 {
		ui.expand_to_include_y(text_line.bottom() + slack);
	}
}
// Discord snowflakes carry milliseconds since 2015-01-01. All u64 IDs fit time's range.
pub(super) fn timestamp(id: Id) -> time::OffsetDateTime {
	crate::local_time::local(
		time::OffsetDateTime::from_unix_timestamp(((id.0 >> 22) / 1000) as i64 + 1_420_070_400)
			.expect("snowflake timestamp is in range"),
	)
}
/// Loaded and private (ephemeral) messages of the selected conversation in id order.
pub(super) fn display_rows<'a>(state: &'a State) -> impl Iterator<Item = &'a Message> + 'a {
	let mut private: Vec<&Message> = state
		.interactions
		.ephemeral
		.iter()
		.filter(|message| Some(message.channel) == state.selected)
		.collect();
	private.sort_by_key(|message| message.id);
	let mut shared = state.timeline.display_iter().peekable();
	let mut private = private.into_iter().peekable();
	std::iter::from_fn(move || match (shared.peek(), private.peek()) {
		(Some(shared_next), Some(private_next)) if private_next.id < shared_next.id => {
			private.next()
		}
		(Some(_), _) => shared.next(),
		(None, _) => private.next(),
	})
}
/// A row payload, including retained deleted bodies and private command responses.
pub(super) fn display_message(state: &State, id: Id) -> Option<&Message> {
	state.timeline.get_display(id).or_else(|| {
		state
			.interactions
			.ephemeral
			.iter()
			.find(|message| message.id == id && Some(message.channel) == state.selected)
	})
}
pub(super) fn queue_missing_channel_reference(
	queued: &mut Option<Id>,
	formatted: &Formatted,
	state: &State,
	revealed: u32,
) {
	if queued.is_none() {
		*queued = formatted.missing_channel_reference(state, revealed);
	}
}
pub(super) fn grouped(previous: Option<&Message>, message: &Message, boundary: Option<Id>) -> bool {
	previous.is_some_and(|previous| {
		previous.author.id == message.author.id
			&& previous.author.account_label() == message.author.account_label()
			&& (previous.author.kind == model::AccountKind::VerifiedBot)
				== (message.author.kind == model::AccountKind::VerifiedBot)
			&& message.reply_to.is_none()
			&& message.interaction.is_none()
			&& !message.ephemeral
			&& !previous.ephemeral
			&& !message.unsupported
			&& !previous.unsupported
			&& !message.extra_content.any()
			&& !previous.extra_content.any()
			&& boundary != Some(message.id)
			&& timestamp(previous.id).date() == timestamp(message.id).date()
			&& (timestamp(message.id) - timestamp(previous.id)).whole_seconds() < 300
	})
}
pub(crate) fn mentions_viewer(message: &Message, state: &State) -> bool {
	let viewer = state.user.as_ref().map(|user| user.id);
	if viewer.is_some_and(|viewer| message.author.id == viewer) {
		return false;
	}
	message.mention_everyone
		|| viewer.is_some_and(|viewer| message.mentions.iter().any(|mention| mention.id == viewer))
		|| (viewer.is_some()
			&& state
				.channel(message.channel)
				.and_then(|channel| channel.guild)
				.and_then(|guild| state.permissions.guilds.get(&guild))
				.and_then(|guild| guild.member.as_ref())
				.is_some_and(|member| {
					message
						.mention_roles
						.iter()
						.any(|role| member.roles.contains(role))
				}))
}
pub(super) fn row_key(
	message: &Message,
	previous: Option<&Message>,
	boundary: Option<Id>,
	state: &State,
) -> u64 {
	let mut key = DefaultHasher::new();
	layout_key(message).hash(&mut key);
	grouped(previous, message, boundary).hash(&mut key);
	previous
		.is_none_or(|previous| timestamp(previous.id).date() != timestamp(message.id).date())
		.hash(&mut key);
	(boundary == Some(message.id)).hash(&mut key);
	crate::mentions::presentation_fingerprint(state, message).hash(&mut key);
	key.finish()
}
pub(super) fn row_height_key(
	message: &Message,
	previous: Option<&Message>,
	boundary: Option<Id>,
	state: &State,
	deleted: bool,
) -> u64 {
	row_key(message, previous, boundary, state) ^ u64::from(deleted)
}
