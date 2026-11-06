use super::*;

/// "N messages · last activity" summary for a thread row, built from synced metadata only.
pub(crate) fn thread_activity(thread: &model::Channel) -> String {
	let count = match thread.message_count {
		Some(0) => crate::i18n::translate("timeline-thread-activity-no-replies"),
		Some(1) => crate::i18n::translate("timeline-thread-activity-one-message"),
		Some(n) => crate::i18n::translate_args(
			"timeline-thread-activity-many-messages",
			&[("count", &n.to_string())],
		),
		None => crate::i18n::translate("timeline-thread-activity-thread"),
	};
	let Some(last) = thread.last_message else {
		return count;
	};
	crate::i18n::translate_args(
		"timeline-thread-activity-last-active",
		&[
			("count", &count),
			("time", &crate::local_time::ago(timestamp(last))),
		],
	)
}
/// A thread card's identity and the message it previews.
pub(super) struct ThreadCard<'a> {
	/// The thread's name, once this session knows its channel.
	pub(super) name: Option<&'a str>,
	/// The service's message count for the thread.
	pub(super) count: Option<u32>,
	/// The thread's latest message, once its bounded read landed.
	pub(super) last: Option<&'a Message>,
	/// One safe line of that message, with mentions named and spoilers hidden.
	pub(super) text: Option<&'a str>,
	/// The latest message this session knows of, so the card shows its time before the body.
	pub(super) latest: Option<Id>,
}
/// "3 messages" for a thread card, phrased like the archive cards.
pub(super) fn thread_count(count: u32) -> String {
	match count {
		0 => crate::i18n::translate("timeline-thread-activity-no-replies"),
		1 => crate::i18n::translate("timeline-thread-activity-one-message"),
		count => crate::i18n::translate_args(
			"timeline-thread-activity-many-messages",
			&[("count", &count.to_string())],
		),
	}
}
/// One safe line of a thread's latest message: mentions by name, hidden spoilers, and what a
/// message without text carries. `channel` is the channel the card sits in, which resolves the
/// guild's nicks and roles even when the thread itself is not loaded.
pub(super) fn thread_preview_text(
	ui: &egui::Ui,
	state: &State,
	formatted: &mut FormatCache,
	channel: Id,
	message: &Message,
) -> String {
	if crate::embeds::has_spoilers(message) {
		return "Spoiler".to_owned();
	}
	let text = if message.is_system() {
		// A service row carries its meaning in the summary, not in the raw content.
		crate::i18n::message_text(message).into_owned()
	} else {
		let mut job = egui::text::LayoutJob::default();
		let source = crate::mentions::MentionSource { state, channel };
		let emojis = formatted
			.get(message.id, &message.content)
			.append_inline_preview(
				&mut job,
				ui,
				&message.mentions,
				Some(&source),
				crate::mentions::known_roles(state, channel),
				&state.channels,
			);
		Formatted::inline_preview_text(&job, &emojis)
	};
	let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
	if !text.is_empty() {
		return text;
	}
	if let Some(attachment) = message.attachments.first() {
		return attachment.filename.clone();
	}
	if let Some(embed) = message.embeds.first() {
		let one_line = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
		if let Some(title) = embed.title.as_deref() {
			let title = one_line(title);
			if !title.is_empty() {
				return title;
			}
		}
		if let Some(description) = embed.description.as_deref() {
			let description = one_line(description);
			if !description.is_empty() {
				return description;
			}
		}
		if let Some(url) = embed.url.as_deref() {
			return url.to_owned();
		}
	}
	if let Some(sticker) = message.sticker_items.first() {
		return sticker.name.clone();
	}
	if let Some(poll) = message.poll.as_deref() {
		return poll
			.question
			.split_whitespace()
			.collect::<Vec<_>>()
			.join(" ");
	}
	String::new()
}
/// The rule that ties a starter's avatar to the thread card under its message, as Discord draws
/// it: down the gutter, around a rounded corner, into the card's left edge.
pub(super) fn thread_connector(
	ui: &egui::Ui,
	avatar: egui::Rect,
	card: egui::Rect,
	colors: &crate::design::Palette,
) {
	let x = avatar.center().x;
	let top = avatar.bottom() + 4.0;
	let y = card.center().y;
	if y <= top + 6.0 || card.left() <= x + 6.0 {
		return;
	}
	let stroke = egui::Stroke::new(2.0, colors.muted.gamma_multiply(0.5));
	ui.painter()
		.line_segment([egui::pos2(x, top), egui::pos2(x, y - 6.0)], stroke);
	ui.painter()
		.add(egui::epaint::QuadraticBezierShape::from_points_stroke(
			[
				egui::pos2(x, y - 6.0),
				egui::pos2(x, y),
				egui::pos2(x + 6.0, y),
			],
			false,
			egui::Color32::TRANSPARENT,
			stroke,
		));
	ui.painter().line_segment(
		[egui::pos2(x + 6.0, y), egui::pos2(card.left() - 1.0, y)],
		stroke,
	);
}
/// Discord-style card under a message that started a thread: the thread's name and message
/// count, then who spoke last and what they said. The whole card opens the thread; its height
/// is fixed, so a read that lands later only fills the card instead of moving the rows below it.
pub(super) fn thread_card(
	ui: &mut egui::Ui,
	state: &State,
	avatars: &mut crate::avatars::Avatars,
	card: ThreadCard<'_>,
	colors: &crate::design::Palette,
) -> (egui::Response, bool) {
	let width = ui.available_width();
	let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 64.0), egui::Sense::click());
	let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
	let visible = ui.is_rect_visible(rect);
	if visible {
		let frame = crate::design::interactive_card_frame(ui, &response);
		ui.painter().add(frame.paint(rect));
		let left = rect.left() + 48.0;
		let right = rect.right() - 16.0;
		let inner = (right - left).max(1.0);
		let icon = egui::Rect::from_center_size(
			egui::pos2(rect.left() + 26.0, rect.center().y),
			egui::Vec2::splat(20.0),
		);
		crate::icons::paint(ui.painter(), crate::icons::Icon::Thread, icon, colors.muted);
		let count = card.count.map(|count| {
			// An empty thread has no count yet; a chevron would promise a list it cannot open.
			let (label, color) = if count == 0 {
				(thread_count(0), colors.muted)
			} else {
				(format!("{} ›", thread_count(count)), colors.link)
			};
			egui::WidgetText::from(RichText::new(label).size(12.5).color(color)).into_galley(
				ui,
				Some(egui::TextWrapMode::Truncate),
				inner,
				egui::FontSelection::Default,
			)
		});
		let title = card.name.map(|name| {
			let reserved = count.as_ref().map_or(0.0, |count| count.size().x + 8.0);
			egui::WidgetText::from(
				crate::design::semibold(ui, name, 14.5).color(colors.text_strong),
			)
			.into_galley(
				ui,
				Some(egui::TextWrapMode::Truncate),
				(inner - reserved).max(40.0),
				egui::FontSelection::Default,
			)
		});
		let time = card.latest.filter(|id| id.0 != 0).map(|id| {
			egui::WidgetText::from(
				RichText::new(crate::local_time::ago(timestamp(id)))
					.size(12.0)
					.color(colors.muted),
			)
			.into_galley(
				ui,
				Some(egui::TextWrapMode::Truncate),
				inner,
				egui::FontSelection::Default,
			)
		});
		// The preview line: the author's avatar and name, then the message's first line.
		let last = card.last.map(|message| {
			let color = state
				.message_author_colors(message)
				.map_or(colors.text_strong, |role| {
					crate::design::role_name_color(role.primary, colors.chat, colors.text_strong)
				});
			let author = egui::WidgetText::from(
				crate::design::medium(ui, state.message_author_name(message), 12.5).color(color),
			)
			.into_galley(
				ui,
				Some(egui::TextWrapMode::Truncate),
				(inner * 0.45).max(40.0),
				egui::FontSelection::Default,
			);
			let text_left = 28.0 + author.size().x + 8.0;
			let text_width =
				(inner - text_left - time.as_ref().map_or(0.0, |time| time.size().x) - 12.0)
					.max(24.0);
			let text = egui::WidgetText::from(
				RichText::new(card.text.unwrap_or_default())
					.size(12.5)
					.color(colors.muted),
			)
			.into_galley(
				ui,
				Some(egui::TextWrapMode::Truncate),
				text_width,
				egui::FontSelection::Default,
			);
			(author, color, text, text_left)
		});
		let title_height = title.as_ref().map_or(0.0, |title| title.size().y);
		let title_y = match (title.as_ref(), last.as_ref()) {
			(Some(_), Some(_)) => rect.top() + 9.0,
			(Some(_), None) => rect.center().y - title_height / 2.0,
			_ => 0.0,
		};
		let preview_y = match (title.as_ref(), last.as_ref()) {
			(Some(_), Some(_)) => rect.top() + 33.0,
			(None, Some(_)) => rect.center().y - 10.0,
			_ => 0.0,
		};
		if let Some(title) = title {
			ui.painter()
				.galley(egui::pos2(left, title_y), title.clone(), colors.text_strong);
			if let Some(count) = count {
				ui.painter().galley(
					egui::pos2(
						left + title.size().x + 8.0,
						title_y + (title.size().y - count.size().y) / 2.0,
					),
					count.clone(),
					colors.link,
				);
			}
			// A card whose preview has not landed yet still shows when the thread was last used.
			if last.is_none()
				&& let Some(time) = &time
			{
				ui.painter().galley(
					egui::pos2(
						right - time.size().x,
						title_y + (title.size().y - time.size().y) / 2.0,
					),
					time.clone(),
					colors.muted,
				);
			}
		}
		if let (Some(message), Some((author, color, text, text_left))) = (card.last, last) {
			avatars.paint_user(
				ui,
				&message.author,
				20.0,
				egui::Rect::from_min_size(egui::pos2(left, preview_y), egui::Vec2::splat(20.0)),
				state.demo,
			);
			let baseline = preview_y + (20.0 - author.size().y) / 2.0;
			ui.painter()
				.galley(egui::pos2(left + 28.0, baseline), author.clone(), color);
			ui.painter().galley(
				egui::pos2(left + text_left, baseline),
				text.clone(),
				colors.muted,
			);
			if let Some(time) = time {
				ui.painter().galley(
					egui::pos2(
						right - time.size().x,
						preview_y + (20.0 - time.size().y) / 2.0,
					),
					time.clone(),
					colors.muted,
				);
			}
		}
	}
	let label = card.name.map_or_else(
		|| crate::i18n::translate("timeline-thread-activity-thread"),
		str::to_owned,
	);
	// The card is painted, so its parts exist only as shapes; name them for screen readers.
	let mut named = Vec::new();
	if let Some(name) = card.name {
		named.push(name.to_owned());
	}
	if let Some(count) = card.count {
		named.push(thread_count(count));
	}
	if let (Some(last), Some(text)) = (card.last, card.text) {
		named.push(format!("{}: {text}", state.message_author_name(last)));
	}
	if let Some(latest) = card.latest {
		named.push(crate::local_time::ago(timestamp(latest)));
	}
	let named = named.join(" · ");
	response.widget_info(|| {
		egui::WidgetInfo::labeled(egui::Role::Link, ui.is_enabled(), named.clone())
	});
	(
		response.on_hover_text(crate::i18n::translate_args(
			"timeline-open-thread",
			&[("thread", &label)],
		)),
		visible,
	)
}
/// The message a thread hangs off, shown above its replies like Discord's thread view.
pub(super) fn starter_row(
	ui: &mut egui::Ui,
	message: &Message,
	state: &State,
	avatars: &mut crate::avatars::Avatars,
	width: f32,
) {
	let colors = crate::design::palette(ui);
	egui::Frame::NONE
		.inner_margin(egui::Margin {
			left: 16,
			right: 16,
			top: group_gap(false),
			bottom: 6,
		})
		.show(ui, |ui| {
			ui.set_min_width((width - 32.0).max(1.0));
			ui.spacing_mut().item_spacing = egui::vec2(16.0, 4.0);
			ui.horizontal_top(|ui| {
				avatars.show_plain(ui, &message.author, 40.0, state.demo);
				ui.vertical(|ui| {
					ui.set_width(ui.available_width());
					ui.allocate_ui_with_layout(
						egui::vec2(ui.available_width(), 22.0),
						egui::Layout::left_to_right(egui::Align::Center),
						|ui| {
							ui.spacing_mut().item_spacing.x = 8.0;
							crate::account_badge::name(
								ui,
								&message.author,
								state.message_author_name(message),
								15.5,
								(
									colors.text_strong,
									state.message_author_colors(message),
									colors.chat,
								),
								egui::Sense::hover(),
								0.0,
							);
							let time = timestamp(message.id);
							ui.label(
								RichText::new(crate::local_time::header(time))
									.size(12.0)
									.color(colors.muted),
							)
							.on_hover_text_with(|| crate::local_time::full(time));
						},
					);
					ui.add(
						egui::Label::new(
							RichText::new(crate::i18n::message_text(message)).color(colors.text),
						)
						.wrap()
						.selectable(false),
					);
				});
			});
			ui.add_space(10.0);
			// A labelled rule separates the starter from the replies that followed it.
			ui.horizontal(|ui| {
				ui.spacing_mut().item_spacing.x = 8.0;
				let (line, _) = ui.allocate_exact_size(
					egui::vec2((ui.available_width() - 220.0).max(24.0), 1.0),
					egui::Sense::hover(),
				);
				ui.painter().hline(
					line.x_range(),
					line.center().y,
					egui::Stroke::new(1.0, colors.border),
				);
				crate::icons::inline(ui, crate::icons::Icon::Thread, 14.0, colors.muted);
				ui.label(
					RichText::new(crate::i18n::translate(
						"timeline-starter-row-thread-started-from-this-message",
					))
					.size(12.0)
					.color(colors.muted),
				);
				let (line, _) = ui.allocate_exact_size(
					egui::vec2(ui.available_width(), 1.0),
					egui::Sense::hover(),
				);
				ui.painter().hline(
					line.x_range(),
					line.center().y,
					egui::Stroke::new(1.0, colors.border),
				);
			});
		});
}
