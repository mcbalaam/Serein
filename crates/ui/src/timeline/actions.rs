use super::*;

/// A hover-bar reaction: its emoji, whether the reader already reacted, and whether it toggles now.
pub(super) fn quick_reaction(
	state: &State,
	message: &Message,
	text: &str,
) -> (model::ReactionEmoji, bool, bool) {
	let emoji = model::ReactionEmoji {
		id: None,
		name: Some(text.to_owned()),
	};
	let reactions = state.reactions.display(message);
	let reacted = reactions
		.and_then(|items| items.iter().find(|r| r.emoji.same(&emoji)))
		.is_some_and(|r| r.me);
	let enabled = reactions.is_some()
		&& !state.reactions.busy()
		&& state.can_react(message.id, Some(&emoji), !reacted);
	(emoji, reacted, enabled)
}
pub(super) fn action_button(
	ui: &mut egui::Ui,
	icon: crate::icons::Icon,
	label: &str,
) -> egui::Response {
	crate::icons::button_immediate(ui, icon, 28.0, label)
}
/// One transient confirmation for all direct copy actions; never retains copied text.
pub(super) fn copy_action_button(
	ui: &mut egui::Ui,
	icon: crate::icons::Icon,
	label: &str,
	key: egui::Id,
	feedback: &mut Option<(egui::Id, f64)>,
	text: impl FnOnce() -> String,
) -> egui::Response {
	let now = ui.input(|input| input.time);
	if feedback.is_some_and(|(_, at)| now - at >= 1.5) {
		*feedback = None;
	}
	let age = feedback
		.filter(|(id, _)| *id == key)
		.map(|(_, at)| now - at);
	let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(28.0), egui::Sense::click());
	let clicked = response.clicked();
	if clicked {
		ui.ctx().copy_text(text());
		*feedback = Some((key, now));
	}
	let age = if clicked { Some(0.0) } else { age };
	let colors = crate::design::palette(ui);
	if response.hovered() || response.has_focus() {
		ui.painter().rect_filled(rect, 6, colors.hover);
	}
	let label = crate::i18n::translate(if age.is_some() {
		"updates-update-settings-copied"
	} else {
		label
	});
	let (icon, scale, color) = if let Some(age) = age {
		let t = (age as f32 / 0.2).clamp(0.0, 1.0);
		let scale =
			0.65 + 0.35 * (1.0 - (1.0 - t).powi(3)) + 0.12 * (std::f32::consts::PI * t).sin();
		if age < 0.2 {
			ui.ctx().request_repaint();
		} else {
			ui.ctx()
				.request_repaint_after(std::time::Duration::from_secs_f64(1.5 - age));
		}
		(crate::icons::Icon::Check, scale, colors.positive)
	} else {
		(
			icon,
			1.0,
			if response.hovered() || response.has_focus() {
				colors.text_strong
			} else {
				colors.muted
			},
		)
	};
	crate::icons::paint(
		ui.painter(),
		icon,
		egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(16.8 * scale)),
		color,
	);
	response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
	crate::icons::immediate_tooltip(response, &label)
}
#[cfg(all(debug_assertions, feature = "demo"))]
pub fn debug_copy_feedback_check() {
	let ctx = egui::Context::default();
	crate::design::apply(&ctx);
	let key = egui::Id::unique("debug-copy");
	let mut feedback = None;
	let mut frame = |time, events| {
		ctx.run_ui(
			egui::RawInput {
				time: Some(time),
				events,
				..Default::default()
			},
			|ui| {
				copy_action_button(
					ui,
					crate::icons::Icon::Copy,
					"message-menu-copy",
					key,
					&mut feedback,
					|| "Synthetic copied message".into(),
				);
			},
		)
	};
	let initial = frame(0.0, vec![]);
	let point = egui::pos2(14.0, 14.0);
	frame(
		0.1,
		vec![
			egui::Event::PointerMoved(point),
			egui::Event::PointerButton {
				pos: point,
				button: egui::PointerButton::Primary,
				pressed: true,
				modifiers: egui::Modifiers::SHIFT,
			},
		],
	)
	.drop_without_applying_deltas();
	let copied = frame(
		0.2,
		vec![egui::Event::PointerButton {
			pos: point,
			button: egui::PointerButton::Primary,
			pressed: false,
			modifiers: egui::Modifiers::SHIFT,
		}],
	);
	assert!(copied.platform_output.commands.iter().any(|command|
		matches!(command, egui::OutputCommand::CopyText(text) if text == "Synthetic copied message")));
	let settled = frame(0.5, vec![]);
	let expired = frame(1.8, vec![]);
	let icon = |output: &egui::FullOutput| {
		output
			.shapes
			.iter()
			.find_map(|shape| match &shape.shape {
				egui::Shape::Mesh(mesh) => Some((mesh.vertices[0].uv, mesh.calc_bounds().width())),
				_ => None,
			})
			.expect("copy action icon")
	};
	assert_ne!(
		icon(&initial).0,
		icon(&copied).0,
		"copy changes to a checkmark"
	);
	assert!(
		icon(&copied).1 < icon(&settled).1,
		"checkmark grows into place"
	);
	assert_eq!(
		icon(&initial).0,
		icon(&expired).0,
		"copy icon returns after confirmation"
	);
	assert!(settled.platform_output.commands.is_empty());
	assert!(expired.platform_output.commands.is_empty());
	for output in [initial, copied, settled, expired] {
		output.drop_without_applying_deltas();
	}
	println!("Copy action clipboard, animated checkmark and expiry checks passed.");
}
pub(super) enum DeletedLocalAction {
	ToggleHighlight,
	Remove,
}
pub(super) fn deleted_message_actions(
	popup: egui::Popup<'_>,
	action: &mut Option<DeletedLocalAction>,
) {
	popup.show(|ui| {
		ui.set_min_width(160.0);
		if ui
			.button(crate::i18n::translate(
				"timeline-deleted-message-actions-toggle-deleted-highlight",
			))
			.clicked()
		{
			*action = Some(DeletedLocalAction::ToggleHighlight);
			ui.close();
		}
		if ui
			.button(crate::i18n::translate(
				"timeline-deleted-message-actions-remove-message",
			))
			.clicked()
		{
			*action = Some(DeletedLocalAction::Remove);
			ui.close();
		}
	});
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn message_actions(
	popup: egui::Popup<'_>,
	(message, link, extension_actions, extension_request): (
		&Message,
		Option<&str>,
		&[crate::extensions_ui::MenuAction],
		&mut Option<(crate::extensions_ui::MenuAction, String)>,
	),
	actions: (bool, bool, bool, bool),
	selection: (
		Option<&mut Option<Id>>,
		Option<&mut Option<Id>>,
		&mut Option<Id>,
	),
	editing: (&mut Option<(Id, Id, String)>, &mut bool),
	deleting: &mut Option<(Id, Id)>,
	pin: (bool, bool, &mut Option<(Id, Id, bool)>),
	thread: (bool, &mut Option<(Id, Id)>),
	forward: (bool, &mut Option<Id>),
	view_reactions: Option<(
		model::ReactionEmoji,
		&mut Option<(Id, model::ReactionEmoji, bool)>,
	)>,
) {
	let (mark_read, mark_unread, reply) = selection;
	let (editing, edit_started) = editing;
	let (own, can_reply, can_edit, can_delete) = actions;
	let (can_pin, pinned, pin_request) = pin;
	let (can_thread, thread_request) = thread;
	popup.show(|ui| {
		ui.set_min_width(160.0);
		if !extension_actions.is_empty() {
			ui.menu_button(
				crate::i18n::translate("timeline-message-actions-extensions"),
				|ui| {
					for action in extension_actions {
						if ui.button(&action.label).clicked() {
							*extension_request =
								Some((action.clone(), message.display_text().into_owned()));
							ui.close();
						}
					}
				},
			);
			ui.separator();
		}
		if crate::select::has_selection(ui.ctx())
			&& ui
				.button(crate::i18n::translate("timeline-message-actions-copy"))
				.clicked()
		{
			crate::select::request_copy(ui.ctx());
			ui.close();
		}
		if ui
			.button(crate::i18n::translate("message-menu-copy"))
			.clicked()
		{
			ui.ctx()
				.copy_text(crate::i18n::message_text(message).into_owned());
			ui.close();
		}
		if let Some(link) = link
			&& ui
				.button(crate::i18n::translate(
					"timeline-message-actions-copy-message-link",
				))
				.clicked()
		{
			ui.ctx().copy_text(link.to_owned());
			ui.close();
		}
		if ui
			.button(crate::i18n::translate(
				"timeline-message-actions-copy-message-id",
			))
			.clicked()
		{
			ui.ctx().copy_text(message.id.to_string());
			ui.close();
		}
		if ui
			.add_enabled(
				can_reply,
				egui::Button::new(crate::i18n::translate("timeline-message-actions-reply")),
			)
			.clicked()
		{
			*reply = Some(message.id);
			ui.close();
		}
		if ui
			.add_enabled(
				forward.0,
				egui::Button::new(crate::i18n::translate("timeline-message-actions-forward")),
			)
			.clicked()
		{
			*forward.1 = Some(message.id);
			ui.close();
		}
		if can_thread
			&& ui
				.button(crate::i18n::translate(
					"timeline-message-actions-create-thread",
				))
				.clicked()
		{
			*thread_request = Some((message.channel, message.id));
			ui.close();
		}
		if let Some((emoji, view)) = view_reactions
			&& ui
				.button(crate::i18n::translate(
					"timeline-message-actions-view-reactions",
				))
				.clicked()
		{
			*view = Some((message.id, emoji, true));
			ui.close();
		}
		if ui
			.add_enabled(
				mark_read.is_some(),
				egui::Button::new(crate::i18n::translate(
					"timeline-message-actions-mark-read-through-here",
				)),
			)
			.clicked()
		{
			if let Some(mark_read) = mark_read {
				*mark_read = Some(message.id);
			}
			ui.close();
		}
		if ui
			.add_enabled(
				mark_unread.is_some(),
				egui::Button::new(crate::i18n::translate(
					"timeline-message-actions-mark-unread",
				)),
			)
			.clicked()
		{
			if let Some(mark_unread) = mark_unread {
				*mark_unread = Some(message.id);
			}
			ui.close();
		}
		if ui
			.add_enabled(
				can_pin,
				egui::Button::new(crate::i18n::translate_if_key(if pinned {
					"timeline-message-actions-unpin-message"
				} else {
					"timeline-message-actions-pin-message"
				})),
			)
			.clicked()
		{
			*pin_request = Some((message.channel, message.id, !pinned));
			ui.close();
		}
		if own || can_delete {
			ui.separator();
		}
		if own
			&& ui
				.add_enabled(
					can_edit,
					egui::Button::new(crate::i18n::translate(
						"timeline-message-actions-edit-message",
					)),
				)
				.clicked()
		{
			*editing = Some((message.channel, message.id, message.content.clone()));
			*edit_started = true;
			ui.close();
		}
		if (own || can_delete)
			&& ui
				.add_enabled(
					can_delete,
					egui::Button::new(crate::i18n::translate(
						"timeline-message-actions-delete-message",
					)),
				)
				.clicked()
		{
			*deleting = Some((message.channel, message.id));
			ui.close();
		}
	});
}
