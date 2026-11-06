use super::*;

/// Discord's Nitro boost pink; not part of any theme palette.
pub(super) const BOOST: egui::Color32 = egui::Color32::from_rgb(0xff, 0x73, 0xfa);
/// Gutter glyph and tint for a system message type, mirroring Discord's system rows.
pub(super) fn system_icon(
	kind: u8,
	colors: &crate::design::Palette,
) -> (crate::icons::Icon, egui::Color32) {
	use crate::icons::Icon;
	match kind {
		1 | 7 => (Icon::ArrowRight, colors.positive),
		2 => (Icon::ArrowLeft, colors.danger),
		3 | 65 => (Icon::Phone, colors.positive),
		4 => (Icon::Pencil, colors.muted),
		5 => (Icon::Image, colors.muted),
		6 => (Icon::Pin, colors.muted),
		8..=11 => (Icon::Sparkle, BOOST),
		12 | 27..=31 => (Icon::Megaphone, colors.muted),
		14 | 15 => (Icon::Compass, colors.positive),
		16 | 17 => (Icon::Compass, colors.warning),
		18 | 21 => (Icon::Thread, colors.muted),
		22 => (Icon::AddPeople, colors.muted),
		24 | 36 | 38 => (Icon::ShieldWarning, colors.danger),
		37 | 39 | 62 => (Icon::ShieldWarning, colors.positive),
		58 => (Icon::Trash, colors.muted),
		59..=61 => (Icon::ShieldWarning, colors.danger),
		55 => (Icon::ScreenShare, colors.accent),
		67 => (Icon::Check, colors.positive),
		25 | 26 | 32 => (Icon::Crown, colors.warning),
		44 => (Icon::ShoppingCart, colors.accent),
		46 => (Icon::ChartBar, colors.muted),
		_ => (Icon::Help, colors.muted),
	}
}
/// Discord-style system row: muted sentence, strong clickable names, inline timestamp.
#[allow(clippy::too_many_arguments)]
pub(super) fn show_system(
	ui: &mut egui::Ui,
	system: &model::SystemMessage,
	time: time::OffsetDateTime,
	state: &State,
	profile: &mut crate::profiles::ProfileSession,
	user_action: &mut Option<crate::user_menu::Action>,
	surface: &mut crate::select::Surface,
	// A "started a thread" row: the known thread, its channel, and where clicks go.
	thread: Option<(Option<Id>, Id)>,
	targets: (&mut Option<Id>, &mut Option<Id>),
) {
	let (open_thread, open_all) = targets;
	{
		let colors = crate::design::palette(ui);
		ui.horizontal_wrapped(|ui| {
			ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
			for segment in &crate::i18n::system_segments(system) {
				if !segment.strong {
					let (pos, galley, response) =
						egui::Label::new(RichText::new(&segment.text).color(colors.muted))
							.wrap()
							.selectable(false)
							.layout_in_ui(ui);
					surface.run(ui, &response, pos, galley, Vec::new());
					continue;
				}
				let text = crate::design::medium(ui, &segment.text, 15.0).color(colors.text_strong);
				let Some(user) = &segment.user else {
					// The thread name in a "started a thread" row opens the thread itself.
					if let Some((Some(id), _)) = thread {
						let response = ui
							.add(egui::Label::new(text).sense(egui::Sense::click()))
							.on_hover_cursor(egui::CursorIcon::PointingHand)
							.on_hover_text(crate::i18n::translate_args(
								"timeline-open-thread",
								&[("thread", &segment.text)],
							));
						surface.keep(&response);
						if response.clicked() {
							*open_thread = Some(id);
						}
						continue;
					}
					let (pos, galley, response) = egui::Label::new(text)
						.wrap()
						.selectable(false)
						.layout_in_ui(ui);
					surface.run(ui, &response, pos, galley, Vec::new());
					continue;
				};
				let response = ui
					.add(egui::Label::new(text).sense(egui::Sense::click()))
					.on_hover_cursor(egui::CursorIcon::PointingHand);
				surface.keep(&response);
				crate::user_menu::show(&response, state, user, profile, user_action);
				profile.person_click(ui, &response, None, user);
			}
			if let Some((_, parent)) = thread {
				let (pos, galley, response) = egui::Label::new(
					RichText::new(crate::i18n::translate("timeline-show-system-see-all"))
						.color(colors.muted),
				)
				.wrap()
				.selectable(false)
				.layout_in_ui(ui);
				surface.run(ui, &response, pos, galley, Vec::new());
				ui.add_space(4.0);
				let all = ui
					.add(
						egui::Label::new(
							crate::design::medium(
								ui,
								crate::i18n::translate("timeline-threads"),
								15.0,
							)
							.color(colors.text_strong),
						)
						.sense(egui::Sense::click()),
					)
					.on_hover_cursor(egui::CursorIcon::PointingHand)
					.on_hover_text(crate::i18n::translate(
						"timeline-show-system-open-this-channels-threads",
					));
				surface.keep(&all);
				if all.clicked() {
					*open_all = Some(parent);
				}
				let (pos, galley, response) =
					egui::Label::new(RichText::new(".").color(colors.muted))
						.wrap()
						.selectable(false)
						.layout_in_ui(ui);
				surface.run(ui, &response, pos, galley, Vec::new());
			}
			ui.add_space(8.0);
			let stamp = ui
				.label(
					RichText::new(crate::local_time::header(time))
						.size(12.0)
						.color(colors.muted),
				)
				.on_hover_text_with(|| crate::local_time::full(time));
			surface.exclude(stamp.rect);
		});
	}
}
