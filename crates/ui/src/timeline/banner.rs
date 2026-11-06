/// Flat strip painted over the timeline edge; `add` lays out its contents left to right.
pub(super) fn overlay_bar(
	ui: &mut egui::Ui,
	rect: egui::Rect,
	fill: egui::Color32,
	radius: egui::CornerRadius,
	add: impl FnOnce(&mut egui::Ui),
) {
	if radius.sw == 0 {
		// Bottom bars cast a soft shadow upward onto the messages behind them.
		ui.painter().rect_filled(
			rect.expand2(egui::vec2(1.0, 0.0))
				.translate(egui::vec2(0.0, -1.0)),
			egui::CornerRadius {
				nw: 9,
				ne: 9,
				sw: 0,
				se: 0,
			},
			egui::Color32::from_black_alpha(48),
		);
	}
	ui.painter().rect_filled(rect, radius, fill);
	let mut bar = ui.new_child(
		egui::UiBuilder::new()
			.scope_id(ui.make_persistent_id(("timeline-overlay", radius.sw == 0)))
			.max_rect(rect.shrink2(egui::vec2(12.0, 0.0)))
			.layout(egui::Layout::left_to_right(egui::Align::Center)),
	);
	bar.spacing_mut().item_spacing.x = 8.0;
	add(&mut bar);
}
/// "Back to the live edge" control floating over the bottom-right of the conversation: a round
/// arrow, or an accent pill naming the messages that arrived below while the reader was away.
pub(super) fn present_control(
	ui: &mut egui::Ui,
	area: egui::Rect,
	unread: bool,
	new_below: Option<&str>,
) -> (egui::Rect, bool) {
	let colors = crate::design::palette(ui);
	let size = 38.0;
	let galley = new_below.map(|label| {
		egui::WidgetText::from(crate::design::medium(ui, label, 14.0).color(colors.accent_text))
			.into_galley(
				ui,
				Some(egui::TextWrapMode::Truncate),
				(area.width() - 32.0 - size).max(size),
				egui::TextStyle::Body,
			)
	});
	let width = galley
		.as_ref()
		.map_or(size, |galley| 16.0 + galley.size().x + 6.0 + 18.0 + 12.0);
	let rect = egui::Rect::from_min_size(
		egui::pos2(
			area.right() - 16.0 - width,
			area.bottom() - crate::typing::OVERLAY_HEIGHT - 10.0 - size,
		),
		egui::vec2(width, size),
	);
	let response = ui.interact(
		rect,
		ui.make_persistent_id("timeline-present"),
		egui::Sense::click(),
	);
	let painter = ui.painter();
	let radius = egui::CornerRadius::same((size / 2.0) as u8);
	painter.rect_filled(
		rect.translate(egui::vec2(0.0, 1.5)),
		radius,
		egui::Color32::from_black_alpha(52),
	);
	let accent = unread || galley.is_some();
	let fill = if accent {
		colors.accent
	} else {
		colors.raised.to_opaque()
	};
	let fill = if response.hovered() {
		fill.gamma_multiply(1.12)
	} else {
		fill
	};
	painter.rect_filled(rect, radius, fill);
	if !accent {
		painter.rect_stroke(
			rect,
			radius,
			egui::Stroke::new(1.0, colors.border),
			egui::StrokeKind::Inside,
		);
	}
	let color = if accent {
		colors.accent_text
	} else {
		colors.text_strong
	};
	let arrow_center = if let Some(galley) = &galley {
		let text_pos = egui::pos2(rect.left() + 16.0, rect.center().y - galley.size().y / 2.0);
		painter.galley(text_pos, galley.clone(), color);
		egui::pos2(rect.right() - 12.0 - 9.0, rect.center().y)
	} else {
		rect.center()
	};
	crate::icons::paint(
		painter,
		crate::icons::Icon::ArrowDown,
		egui::Rect::from_center_size(arrow_center, egui::Vec2::splat(18.0)),
		color,
	);
	if response.has_focus() {
		painter.rect_stroke(
			rect.expand(2.0),
			egui::CornerRadius::same((size / 2.0 + 2.0) as u8),
			egui::Stroke::new(1.0, colors.accent),
			egui::StrokeKind::Outside,
		);
	}
	let name = crate::i18n::translate("timeline-present-control-jump-to-present");
	let name = new_below.map_or(name.clone(), |label| format!("{label} · {name}"));
	response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
	let clicked = response
		.on_hover_text(if unread || new_below.is_some() {
			crate::i18n::translate("timeline-present-control-new-messages-below-jump-to-present")
		} else {
			crate::i18n::translate("timeline-present-control-jump-to-present")
		})
		.clicked();
	(rect, clicked)
}
/// Frameless text action with a trailing arrow glyph, for use inside [`overlay_bar`].
pub(super) fn bar_button(
	ui: &mut egui::Ui,
	label: &str,
	icon: crate::icons::Icon,
	color: egui::Color32,
) -> egui::Response {
	// Right-to-left layouts place the first item at the right edge, so the glyph goes first.
	let rtl = ui.layout().horizontal_placement() == egui::Align::Max;
	let glyph = |ui: &mut egui::Ui| {
		crate::icons::inline(ui, icon, 14.0, color);
	};
	if rtl {
		glyph(ui);
	}
	ui.spacing_mut().item_spacing.x = 4.0;
	let response = ui.add(
		egui::Button::new(crate::design::medium(ui, label, 13.0).color(color))
			.frame(false)
			.small(),
	);
	if !rtl {
		glyph(ui);
	}
	ui.spacing_mut().item_spacing.x = 12.0;
	response
}
pub(super) fn banner_rect(area: egui::Rect) -> egui::Rect {
	egui::Rect::from_min_size(
		egui::pos2(area.left() + 16.0, area.top()),
		egui::vec2((area.width() - 32.0).max(120.0), 28.0),
	)
}
/// Returns whether the reader asked to jump to unread or mark the channel read.
/// Discord's unread bar; `summary` is "N new messages since HH:MM" once the divider is loaded.
pub(super) fn unread_banner(
	ui: &mut egui::Ui,
	rect: egui::Rect,
	jump: bool,
	summary: Option<&str>,
) -> (bool, bool) {
	let colors = crate::design::palette(ui);
	let mut jump_unread = false;
	let mut mark_read = false;
	let text = colors.accent_text;
	overlay_bar(
		ui,
		rect,
		colors.accent,
		egui::CornerRadius {
			nw: 0,
			ne: 0,
			sw: 8,
			se: 8,
		},
		|ui| {
			// Buttons first, so a long summary truncates into the space they leave.
			ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
				let mark = crate::i18n::translate("timeline-unread-banner-mark-as-read");
				if bar_button(ui, &mark, crate::icons::Icon::Check, text).clicked() {
					mark_read = true;
				}
				let jump_label = crate::i18n::translate("timeline-unread-banner-jump-to-unread");
				if jump && bar_button(ui, &jump_label, crate::icons::Icon::ArrowUp, text).clicked()
				{
					jump_unread = true;
				}
				ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
					ui.add(
						egui::Label::new(
							crate::design::medium(
								ui,
								summary.map_or_else(
									|| {
										crate::i18n::translate(
											"timeline-unread-banner-unread-messages",
										)
									},
									str::to_owned,
								),
								13.0,
							)
							.color(text),
						)
						.truncate(),
					);
				});
			});
		},
	);
	(jump_unread, mark_read)
}
