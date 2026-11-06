use super::*;

pub(super) fn channel_welcome(ui: &mut egui::Ui, channel: &model::Channel, height: f32) {
	let colors = crate::design::palette(ui);
	let width = (ui.available_width() - 32.0).max(1.0);
	let heading = egui::WidgetText::from(
		crate::design::semibold(
			ui,
			crate::i18n::translate_args(
				"timeline-channel-welcome-title",
				&[("channel", &channel.name)],
			),
			28.0,
		)
		.color(colors.text_strong),
	)
	.into_galley(
		ui,
		Some(egui::TextWrapMode::Wrap),
		width,
		egui::TextStyle::Heading,
	);
	let description = egui::WidgetText::from(
		RichText::new(crate::i18n::translate(
			"timeline-channel-welcome-this-is-the-beginning-of-the-conversation",
		))
		.color(colors.muted),
	)
	.into_galley(
		ui,
		Some(egui::TextWrapMode::Wrap),
		width,
		egui::TextStyle::Body,
	);
	// Scroll contents have unbounded available height. Use the finite viewport and
	// measured, wrapped text so short channels sit above the composer at every width.
	let content_height = 32.0 + 64.0 + 16.0 + heading.size().y + 8.0 + description.size().y;
	ui.add_space((height - content_height).max(0.0));
	egui::Frame::NONE
		.inner_margin(egui::Margin::same(16))
		.show(ui, |ui| {
			let (badge, _) = ui.allocate_exact_size(egui::Vec2::splat(64.0), egui::Sense::hover());
			ui.painter()
				.circle_filled(badge.center(), 32.0, colors.raised);
			crate::icons::paint(
				ui.painter(),
				match channel.kind {
					5 => crate::icons::Icon::Megaphone,
					10..=12 => crate::icons::Icon::Thread,
					_ => crate::icons::Icon::Hash,
				},
				badge.shrink(14.0),
				colors.text_strong,
			);
			ui.add_space(16.0);
			ui.add(egui::Label::new(heading));
			ui.add_space(8.0);
			ui.add(egui::Label::new(description));
		});
}
pub(super) fn loading_messages(ui: &mut egui::Ui) {
	let colors = crate::design::palette(ui);
	let height = ui.available_height();
	let (rect, response) = ui.allocate_exact_size(
		egui::vec2(ui.available_width(), height),
		egui::Sense::hover(),
	);
	response.widget_info(|| {
		egui::WidgetInfo::labeled(
			egui::Role::Label,
			false,
			crate::i18n::translate("timeline-loading-messages-loading-messages"),
		)
	});
	let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(rect));
	let fill = colors.muted.gamma_multiply(0.22);
	let text_width = (rect.width() - 88.0).clamp(0.0, 480.0);
	let rows = (height / 68.0).ceil().clamp(0.0, 128.0) as usize;
	for (index, length) in [0.85, 0.65, 0.95, 0.55]
		.into_iter()
		.cycle()
		.take(rows)
		.enumerate()
	{
		let origin = rect.min + egui::vec2(16.0, 12.0 + index as f32 * 68.0);
		painter.circle_filled(origin + egui::vec2(20.0, 20.0), 20.0, fill);
		for (y, width, height) in [
			(0.0, text_width.min(96.0), 12.0),
			(22.0, text_width * length, 10.0),
			(40.0, text_width * length * 0.7, 10.0),
		] {
			painter.rect_filled(
				egui::Rect::from_min_size(origin + egui::vec2(56.0, y), egui::vec2(width, height)),
				4,
				fill,
			);
		}
	}
}
/// The curved gutter connector shared by reply and command-invocation headers.
pub(super) fn reference_spine(ui: &mut egui::Ui, colors: &crate::design::Palette) {
	let (gutter, _) = ui.allocate_exact_size(egui::vec2(50.0, 18.0), egui::Sense::hover());
	let x = gutter.left() + 20.0;
	let y = gutter.center().y;
	let stroke = egui::Stroke::new(2.0, colors.muted.gamma_multiply(0.5));
	ui.painter().line_segment(
		[egui::pos2(x, gutter.bottom() + 2.0), egui::pos2(x, y + 5.0)],
		stroke,
	);
	ui.painter()
		.add(egui::epaint::QuadraticBezierShape::from_points_stroke(
			[
				egui::pos2(x, y + 5.0),
				egui::pos2(x, y),
				egui::pos2(x + 5.0, y),
			],
			false,
			egui::Color32::TRANSPARENT,
			stroke,
		));
	ui.painter().line_segment(
		[egui::pos2(x + 5.0, y), egui::pos2(gutter.right(), y)],
		stroke,
	);
}
pub(super) fn divider(ui: &mut egui::Ui, label: String, unread: bool, compact: bool) {
	let colors = crate::design::palette(ui);
	let color = if unread { colors.danger } else { colors.muted };
	ui.add_space(if compact { 8.0 } else { 16.0 });
	ui.horizontal(|ui| {
		ui.add_space(16.0);
		let font = egui::FontId::new(12.0, crate::design::semibold_family(ui.ctx()));
		let text = ui.painter().layout_no_wrap(label.clone(), font, color);
		let (rect, response) = ui.allocate_exact_size(
			egui::vec2((ui.available_width() - 16.0).max(0.0), 20.0),
			crate::select::band_sense(),
		);
		response
			.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, ui.is_enabled(), &label));
		let pos = egui::pos2(
			rect.center().x - text.size().x / 2.0,
			rect.center().y - text.size().y / 2.0,
		);
		let gap = (rect.width() - text.size().x - 24.0).max(0.0) / 2.0;
		for (a, b) in [
			(rect.left(), rect.left() + gap),
			(rect.right() - gap, rect.right()),
		] {
			ui.painter().line_segment(
				[
					egui::pos2(a, rect.center().y),
					egui::pos2(b, rect.center().y),
				],
				egui::Stroke::new(1.0, if unread { color } else { colors.border }),
			);
		}
		egui::text_selection::LabelSelectionState::label_text_selection(
			ui,
			&response,
			pos,
			text,
			color,
			egui::Stroke::NONE,
		);
		if response.hovered() {
			ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
		}
	});
	ui.add_space(if compact { 2.0 } else { 4.0 });
}
