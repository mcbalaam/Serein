use super::helpers::*;
use super::*;

/// Jumped to a conversation's first messages, a window too short to scroll fetches the next
/// newer page by itself instead of waiting for a scroll that cannot happen.
#[test]
fn a_short_window_at_a_target_fetches_newer_messages_by_itself() {
	let mut state = test_support::demo_state();
	state.timeline.clear();
	state.selected = Some(Id(20));
	state.freshness = model::Freshness::Fresh;
	state.history_pending = false;
	state.history_targeted = true;
	state.history_before = Some(Id(3));
	state
		.channels
		.iter_mut()
		.find(|channel| channel.id == Id(20))
		.unwrap()
		.last_message = Some(Id(100));
	for id in 1..=2 {
		state
			.timeline
			.insert(text_message(id), false, false)
			.unwrap();
	}
	state.search_target = Some(Id(2));
	state.revision += 1;
	assert!(state.can_load_newer());
	let ctx = egui::Context::default();
	crate::design::apply(&ctx);
	let mut view = TimelineView::default();
	for _ in 0..4 {
		banner_frame(&ctx, &mut view, &mut state, vec![], false);
	}
	assert!(
		view.fill_newer,
		"the next page is requested without a scroll"
	);
	assert!(!view.load_newer);
}

#[test]
fn downward_wheel_at_pinned_target_loads_next_page() {
	let mut state = test_support::demo_state();
	state.timeline.clear();
	state.selected = Some(Id(20));
	state.freshness = model::Freshness::Fresh;
	state.history_pending = false;
	state.history_targeted = true;
	state.history_before = Some(Id(51));
	state
		.channels
		.iter_mut()
		.find(|channel| channel.id == Id(20))
		.unwrap()
		.last_message = Some(Id(100));
	for id in 1..=50 {
		state
			.timeline
			.insert(text_message(id), false, false)
			.unwrap();
	}
	state.search_target = Some(Id(50));
	state.revision += 1;
	assert!(state.can_load_newer());

	let ctx = egui::Context::default();
	crate::design::apply(&ctx);
	let mut view = TimelineView::default();
	for _ in 0..80 {
		banner_frame(&ctx, &mut view, &mut state, vec![], false);
		if view.reveal_scroll.is_none() && view.highlighted.is_some() {
			break;
		}
	}
	assert!(!view.at_current_latest);

	banner_frame(
		&ctx,
		&mut view,
		&mut state,
		vec![
			egui::Event::PointerMoved(egui::pos2(450.0, 300.0)),
			egui::Event::MouseWheel {
				unit: egui::MouseWheelUnit::Point,
				delta: egui::vec2(0.0, -600.0),
				modifiers: egui::Modifiers::NONE,
				phase: egui::TouchPhase::Move,
				source: egui::MouseWheelSource::Unknown,
			},
		],
		false,
	);

	assert!(view.load_newer);
	assert!(view.target_browsing);
}

#[test]
fn underestimated_leading_row_does_not_hide_history_or_inflate_scroll_extent() {
	fn contains_final_row(shape: &egui::Shape) -> bool {
		match shape {
			egui::Shape::Text(text) => text.galley.job.text.contains("Visible final row"),
			egui::Shape::Vec(shapes) => shapes.iter().any(contains_final_row),
			_ => false,
		}
	}

	let mut state = State {
		selected: Some(Id(20)),
		revision: 1,
		demo: true,
		..Default::default()
	};
	for id in 1..=4 {
		let mut message = text_message(id);
		message.content = if id == 1 {
			"Tall leading row\n".repeat(80)
		} else if id == 4 {
			"Visible final row".into()
		} else {
			"Visible anchor row".into()
		};
		state.timeline.insert(message, false, false).unwrap();
	}
	let ctx = egui::Context::default();
	crate::design::apply(&ctx);
	let mut view = TimelineView::default();
	let mut avatars = crate::avatars::Avatars::default();
	let mut frame_number = 0;
	let mut frame = |view: &mut TimelineView, state: &mut State| {
		frame_number += 1;
		ctx.run_ui(
			egui::RawInput {
				time: Some(f64::from(frame_number) / 60.0),
				screen_rect: Some(egui::Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(900.0, 600.0),
				)),
				..Default::default()
			},
			|ui| {
				view.show(
					ui,
					state,
					&mut None,
					&mut None,
					(
						&mut avatars,
						&mut crate::profiles::ProfileSession::default(),
					),
					None,
				)
			},
		)
	};
	for _ in 0..8 {
		frame(&mut view, &mut state).drop_without_applying_deltas();
	}

	// Force a severe underestimate without invalidating the row key, as can
	// happen when content geometry changes independently of its message data.
	let key = row_key(
		state.timeline.get(Id(1)).unwrap(),
		None,
		view.unread_boundary,
		&state,
	);
	view.heights.insert(Id(1), (key, 76.0));
	view.following = false;
	// The extra short row before the anchor also catches premature cutoff while
	// the leading measurement cursor is still below the visible viewport.
	view.anchor = Some((Id(3), 5.0));
	view.revision = u64::MAX;
	let output = frame(&mut view, &mut state);
	let final_visible = output
		.shapes
		.iter()
		.any(|shape| shape.clip_rect.is_positive() && contains_final_row(&shape.shape));
	output.drop_without_applying_deltas();
	assert!(
		view.heights[&Id(1)].1 > 600.0,
		"Fixture must measure a tall leading row"
	);
	assert!(
		final_visible,
		"Leading measurement must not blank the visible rows"
	);
	assert!(
		view.following,
		"The compensated short content must reach its real bottom; hidden leading bounds must not create phantom scroll space"
	);
}

#[test]
fn wheel_scrolling_keeps_visible_messages_stable_during_measurement() {
	fn texts(shape: &egui::Shape, out: &mut BTreeMap<String, f32>) {
		match shape {
			egui::Shape::Text(text) if text.galley.job.text.starts_with("Row ") => {
				out.insert(text.galley.job.text.clone(), text.pos.y);
			}
			egui::Shape::Vec(shapes) => {
				for shape in shapes {
					texts(shape, out);
				}
			}
			_ => {}
		}
	}
	let mut worst_error = 0.0_f32;
	for width in [900.0, 360.0] {
		let mut state = State {
			selected: Some(Id(20)),
			revision: 1,
			demo: true,
			..Default::default()
		};
		for id in 1..=500 {
			let mut message = text_message(id);
			message.content = format!("Row {id}: {}", message.content);
			if id % 7 == 0 {
				message
					.content
					.push_str(&" Long wrapping content.".repeat(24));
			}
			state.timeline.insert(message, false, false).unwrap();
		}
		let ctx = egui::Context::default();
		crate::design::apply(&ctx);
		let mut view = TimelineView {
			instant_scrolling: true,
			..Default::default()
		};
		let mut avatars = crate::avatars::Avatars::default();
		let mut frame_number = 0;
		let mut frame = |view: &mut TimelineView, delta: f32| {
			frame_number += 1;
			let output = ctx.run_ui(
				egui::RawInput {
					time: Some(f64::from(frame_number) / 60.0),
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(width, 600.0),
					)),
					events: vec![
						egui::Event::PointerMoved(egui::pos2(150.0, 200.0)),
						egui::Event::MouseWheel {
							unit: egui::MouseWheelUnit::Point,
							delta: egui::vec2(0.0, delta),
							modifiers: egui::Modifiers::NONE,
							phase: egui::TouchPhase::Move,
							source: egui::MouseWheelSource::Unknown,
						},
					],
					..Default::default()
				},
				|ui| {
					crate::scroll::apply_preferences(
						ui.ctx(),
						model::ReadingPreferences {
							smooth_scrolling: false,
							..Default::default()
						},
					);
					view.show(
						ui,
						&mut state,
						&mut None,
						&mut None,
						(
							&mut avatars,
							&mut crate::profiles::ProfileSession::default(),
						),
						None,
					)
				},
			);
			let mut labels = BTreeMap::new();
			for shape in &output.shapes {
				if shape.clip_rect.is_positive() {
					texts(&shape.shape, &mut labels);
				}
			}
			output.drop_without_applying_deltas();
			labels
		};
		for _ in 0..8 {
			frame(&mut view, 0.0);
		}
		assert!(view.instant_scrolling);
		view.following = false;
		view.anchor = Some((Id(200), 5.0));
		view.revision = u64::MAX;
		for _ in 0..8 {
			frame(&mut view, 0.0);
		}
		let mut max_error = 0.0_f32;
		// Small point deltas bypass wheel smoothing; each presented frame must move
		// the same message by four points, including frames that discover new rows.
		for (delta, frames) in [(4.0, 120), (-4.0, 240), (0.0, 4)] {
			let mut previous = frame(&mut view, delta);
			let mut comparisons = 0;
			for _ in 0..frames {
				let current = frame(&mut view, delta);
				for (text, y) in &current {
					if (50.0..500.0).contains(y)
						&& let Some(before) = previous.get(text)
					{
						max_error = max_error.max((y - before - delta).abs());
						comparisons += 1;
					}
				}
				previous = current;
			}
			assert!(
				comparisons >= frames,
				"Every frame needs visible message evidence"
			);
		}
		println!("width={width}, maximum scroll displacement error={max_error:.3}pt");
		worst_error = worst_error.max(max_error);
	}
	assert!(
		worst_error < 1.0,
		"wheel movement must not bounce during reflow"
	);
}

#[test]
fn native_layout_virtualizes_preserves_anchor_and_jumps_after_scrolling() {
	for (width, dark) in [(900.0, true), (360.0, false)] {
		let mut state = State {
			selected: Some(Id(20)),
			revision: 1,
			demo: true,
			..Default::default()
		};
		for id in 1..=500 {
			state
				.timeline
				.insert(text_message(id), false, false)
				.unwrap();
		}
		let ctx = egui::Context::default();
		crate::design::apply(&ctx);
		ctx.set_visuals(if dark {
			egui::Visuals::dark()
		} else {
			egui::Visuals::light()
		});
		let mut view = TimelineView::default();
		let mut avatars = crate::avatars::Avatars::default();
		let mut render = |view: &mut TimelineView, state: &mut State| {
			ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(width, 600.0),
					)),
					..Default::default()
				},
				|ui| {
					view.show(
						ui,
						state,
						&mut None,
						&mut None,
						(
							&mut avatars,
							&mut crate::profiles::ProfileSession::default(),
						),
						None,
					);
				},
			)
			.drop_without_applying_deltas();
		};
		for _ in 0..8 {
			render(&mut view, &mut state);
		}
		assert!(view.following);
		assert!(
			view.heights.len() < 60,
			"Only visible rows and overscan are measured, got {}",
			view.heights.len()
		);
		state
			.timeline
			.insert(text_message(501), false, false)
			.unwrap();
		state.revision += 1;
		render(&mut view, &mut state);
		assert!(
			view.following,
			"An arriving message must keep the live edge sticky"
		);
		view.following = false;
		view.anchor = Some((Id(200), 5.0));
		view.revision = u64::MAX;
		for _ in 0..8 {
			render(&mut view, &mut state);
		}
		assert!(!view.following);
		let anchor = view.anchor.unwrap();
		assert_eq!(anchor.0, Id(200));
		// A newly measured leading row while browsing must not opt into the
		// bottom restoration used for a following layout retry.
		let (key, height) = view.heights[&Id(199)];
		assert!(height > 1.0);
		let reflows = view.reflow_frames;
		view.heights.insert(Id(199), (key, 1.0));
		view.revision = u64::MAX;
		render(&mut view, &mut state);
		assert!(view.reflow_frames > reflows);
		assert!(!view.following && !view.jump);
		let restored = view.anchor.expect("anchor kept");
		assert_eq!(restored.0, anchor.0);
		assert!((restored.1 - anchor.1).abs() <= 1.0);
		crate::MessagingUi::default().apply_reading_preferences(
			&ctx,
			model::ReadingPreferences {
				zoom_percent: 125,
				..Default::default()
			},
		);
		for _ in 0..8 {
			render(&mut view, &mut state);
		}
		let zoomed = view.anchor.unwrap().0;
		assert!(
			zoomed == anchor.0 || zoomed == Id(anchor.0.0 - 1),
			"Reading zoom preserves the anchored message: {zoomed:?}"
		);
		state.timeline.insert(text_message(0), false, true).unwrap();
		state.revision += 1;
		for _ in 0..4 {
			render(&mut view, &mut state);
		}
		assert_eq!(view.anchor.unwrap().0, anchor.0);
		state.set_preserve_deleted_messages(true);
		state.timeline.delete(anchor.0).unwrap();
		state.revision += 1;
		for _ in 0..4 {
			render(&mut view, &mut state);
		}
		assert_eq!(
			view.anchor.unwrap().0,
			anchor.0,
			"Deleting the anchored row keeps that message visible"
		);
		assert!(view.heights.contains_key(&anchor.0));
		view.jump = true;
		view.following = true;
		for _ in 0..8 {
			render(&mut view, &mut state);
		}
		assert!(
			view.following,
			"Explicit jump must override persisted scroll state"
		);
	}
}

#[test]
fn switching_cached_chats_keeps_message_positions_stable() {
	for (width, count) in [(900.0, 2), (360.0, 50)] {
		let mut state = test_support::demo_state();
		let ctx = egui::Context::default();
		crate::design::apply(&ctx);
		let mut view = TimelineView::default();
		let mut avatars = crate::avatars::Avatars::default();
		let mut render = |state: &mut State, events: Vec<egui::Event>| {
			let output = ctx.run_ui(
				egui::RawInput {
					events,
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(width, 480.0),
					)),
					..Default::default()
				},
				|ui| {
					view.show(
						ui,
						state,
						&mut None,
						&mut None,
						(
							&mut avatars,
							&mut crate::profiles::ProfileSession::default(),
						),
						None,
					)
				},
			);
			let y = output.shapes.iter().find_map(|shape| match &shape.shape {
				egui::Shape::Text(text) if text.galley.text().contains("Switch anchor") => {
					Some(text.pos.y)
				}
				_ => None,
			});
			output.drop_without_applying_deltas();
			(
				y,
				view.scroll_offset,
				view.following,
				view.hold_read_ack,
				view.anchor,
			)
		};
		for channel in [Id(20), Id(21)] {
			state.select(channel);
			state.history(None);
			let messages = (0..count)
				.map(|index| {
					let mut message = text_message(1000 * channel.0 + index);
					message.channel = channel;
					message.content = if index == count - 1 {
						"Switch anchor".into()
					} else {
						"A wrapped synthetic message with different measured and estimated heights. ".repeat(3)
					};
					message
				})
				.collect();
			state.apply(client_core::Envelope {
				generation: state.generation,
				event: client_core::Event::History {
					channel,
					request: state.request,
					older: false,
					messages,
				},
			});
			for _ in 0..6 {
				render(&mut state, vec![]);
			}
		}
		for channel in [Id(20), Id(21), Id(20)] {
			// A newer message known from channel metadata makes the cached window revalidate.
			state.observe_last_message(channel, Id(1000 * channel.0 + count));
			assert!(matches!(
				state.select(channel),
				Some(client_core::Command::History { .. })
			));
			assert!(state.history_pending);
			let first = render(&mut state, vec![])
				.0
				.expect("cached switch must paint last message");
			for _ in 0..4 {
				let next = render(&mut state, vec![])
					.0
					.expect("settled chat must paint last message");
				assert!(
					(first - next).abs() <= 1.0,
					"cached switch moved from {first} to {next} at width {width}"
				);
			}
			let messages = state.timeline.iter().cloned().collect();
			state.apply(client_core::Envelope {
				generation: state.generation,
				event: client_core::Event::History {
					channel,
					request: state.request,
					older: false,
					messages,
				},
			});
			let refreshed = render(&mut state, vec![])
				.0
				.expect("refreshed chat must paint last message");
			assert!(
				(first - refreshed).abs() <= 1.0,
				"refresh moved from {first} to {refreshed} at width {width}"
			);
		}
		if count == 50 {
			let (_, bottom, _, _, _) = render(&mut state, vec![]);
			let up = vec![
				egui::Event::PointerMoved(egui::pos2(width / 2.0, 240.0)),
				egui::Event::MouseWheel {
					unit: egui::MouseWheelUnit::Point,
					delta: egui::vec2(0.0, 4_000.0),
					modifiers: egui::Modifiers::NONE,
					phase: egui::TouchPhase::Move,
					source: egui::MouseWheelSource::Unknown,
				},
			];
			let mut left = bottom;
			let mut pinned = None;
			for _ in 0..6 {
				let frame = render(&mut state, up.clone());
				left = frame.1;
				pinned = frame.4;
			}
			let (pinned_id, _) = pinned.expect("scrolled channel has an anchor");
			assert!(
				left + 200.0 < bottom,
				"scroll left the live edge at {left}, bottom was {bottom}"
			);
			state.select(Id(21));
			render(&mut state, vec![]);
			assert!(
				state.select(Id(20)).is_none(),
				"a parked page scrolls locally"
			);
			let mut back = (None, 0.0, true, false, None);
			for _ in 0..8 {
				back = render(&mut state, vec![]);
			}
			assert_eq!(back.4.map(|(id, _)| id), Some(pinned_id));
			assert!(
				(back.1 - left).abs() <= 1.0,
				"loaded return moved from {left} to {}",
				back.1
			);
			assert!(!back.2 && !back.3);
			state.select(Id(21));
			render(&mut state, vec![]);
			state.clear_cached_history();
			let command = state.select(Id(20));
			let client_core::Command::History {
				before: None,
				after: Some(after),
				..
			} = command.expect("an evicted page requests history")
			else {
				panic!("evicted return did not request the page after the pinned message");
			};
			let saved = state
				.reading(Id(20))
				.expect("evicted channel keeps its cursor");
			let pinned_id = saved.message.expect("evicted cursor names a message");
			assert_eq!(after.0, pinned_id.0 - 1);
			let pinned_inset = saved.inset;
			let messages = (0..40)
				.map(|index| {
					let mut message = text_message(pinned_id.0 + index);
					message.channel = Id(20);
					message.content = "A wrapped synthetic message with different measured and estimated heights. ".repeat(3);
					message
				})
				.collect();
			state.apply(client_core::Envelope {
				generation: state.generation,
				event: client_core::Event::History {
					channel: Id(20),
					request: state.request,
					older: false,
					messages,
				},
			});
			let mut back = (None, 0.0, true, false, None);
			for _ in 0..8 {
				back = render(&mut state, vec![]);
			}
			assert_eq!(back.4.map(|(id, _)| id), Some(pinned_id));
			assert!((back.1 - pinned_inset).abs() <= 1.0);
			assert!(!back.2 && !back.3);
		}
	}
}

#[test]
fn resident_preview_renders_only_selected_rows_while_revalidating() {
	fn collect(shape: &egui::Shape, labels: &mut Vec<String>) {
		match shape {
			egui::Shape::Text(text) => labels.push(text.galley.job.text.clone()),
			egui::Shape::Vec(shapes) => {
				for shape in shapes {
					collect(shape, labels);
				}
			}
			_ => {}
		}
	}
	for (width, dark) in [(900.0, true), (360.0, false)] {
		for deleted_only in [false, true] {
			let mut state = test_support::demo_state();
			state.timeline.clear();
			state.history(None);
			let messages = (500..550)
				.map(|id| {
					let mut message = text_message(id);
					message.content = format!("Resident alpha row {id}");
					message
				})
				.collect();
			state.apply(client_core::Envelope {
				generation: state.generation,
				event: client_core::Event::History {
					channel: Id(20),
					request: state.request,
					older: false,
					messages,
				},
			});
			if deleted_only {
				state.set_preserve_deleted_messages(true);
				state.apply(client_core::Envelope {
					generation: state.generation,
					event: client_core::Event::DeleteBulk {
						channel: Id(20),
						ids: (500..550).map(Id).collect(),
					},
				});
			}
			let expected: Vec<_> = state.timeline.row_ids().collect();
			assert_eq!(expected.len(), 50);
			let ctx = egui::Context::default();
			crate::design::apply(&ctx);
			ctx.set_visuals(if dark {
				egui::Visuals::dark()
			} else {
				egui::Visuals::light()
			});
			let mut view = TimelineView::default();
			let mut avatars = crate::avatars::Avatars::default();
			let mut render = |view: &mut TimelineView, state: &mut State| {
				let output = ctx.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(width, 480.0),
						)),
						..Default::default()
					},
					|ui| {
						view.show(
							ui,
							state,
							&mut None,
							&mut None,
							(
								&mut avatars,
								&mut crate::profiles::ProfileSession::default(),
							),
							None,
						);
						assert!(ui.min_rect().right() <= ui.max_rect().right() + 1.0);
					},
				);
				assert!(
					output
						.platform_output
						.commands
						.iter()
						.all(|command| matches!(
							command,
							egui::OutputCommand::TextSelectionSettled(_)
						))
				);
				let mut labels = vec![];
				for shape in &output.shapes {
					collect(&shape.shape, &mut labels);
				}
				output.drop_without_applying_deltas();
				labels
			};
			for _ in 0..6 {
				render(&mut view, &mut state);
			}
			assert!(matches!(
				state.select(Id(21)),
				Some(client_core::Command::History {
					channel: Id(21),
					before: None,
					..
				})
			));
			let labels = render(&mut view, &mut state);
			assert!(view.rows.is_empty());
			assert!(!labels.iter().any(|text| text.contains("Resident alpha")));
			let mut beta = text_message(900);
			beta.channel = Id(21);
			beta.content = "Resident beta content".into();
			state.apply(client_core::Envelope {
				generation: state.generation,
				event: client_core::Event::History {
					channel: Id(21),
					request: state.request,
					older: false,
					messages: vec![beta],
				},
			});
			for _ in 0..3 {
				render(&mut view, &mut state);
			}
			assert_eq!(
				view.rows.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
				vec![INTRO, Id(900)]
			);
			// A newer message known from channel metadata makes the window a preview to revalidate.
			state.observe_last_message(Id(20), Id(600));
			assert!(matches!(
				state.select(Id(20)),
				Some(client_core::Command::History {
					channel: Id(20),
					before: None,
					..
				})
			));
			assert_eq!(state.freshness, model::Freshness::Loading);
			assert!(state.history_pending);
			assert_eq!(state.timeline.row_ids().collect::<Vec<_>>(), expected);
			for _ in 0..6 {
				render(&mut view, &mut state);
			}
			let labels = render(&mut view, &mut state);
			assert!(!labels.iter().any(|text| text.contains("Resident beta")));
			assert_eq!(
				view.rows.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
				expected
			);
			assert!(
				view.rows
					.iter()
					.all(|(_, height)| height.is_finite() && *height > 0.0)
			);
			assert!(
				view.following,
				"Resident navigation starts at the latest loaded row"
			);
			assert!(
				view.mark_read.is_none(),
				"Unrevalidated resident rows must not acknowledge read state"
			);
			assert!(labels.iter().any(|text| text.contains("Resident alpha")));
			if deleted_only {
				assert!(state.timeline.is_empty());
				assert!(!labels.iter().any(|text| text == "Message deleted"));
			}
		}
	}
}

#[test]
fn mixed_height_virtualization_visits_only_viewport() {
	let rows: Vec<_> = (1..=500)
		.map(|id| (Id(id), if id % 2 == 0 { 100.0 } else { 40.0 }))
		.collect();
	let (start, end, top) = visible_range(&rows, 1000.0, 1500.0);
	assert!(start > 0);
	assert!(end - start < 12);
	assert!(top <= 1000.0);
	assert_eq!(visible_range(&[], 0.0, 100.0), (0, 0, 0.0));
	let neighbors = [(Id(1), 40.0), (Id(3), 100.0), (Id(4), 60.0)];
	assert_eq!(visible_range(&neighbors, 40.0, 100.0), (1, 2, 40.0));
	assert_eq!(anchor_offset(&neighbors, Id(2), 25.0), 40.0);
	assert_eq!(anchor_offset(&neighbors, Id(5), 25.0), 140.0);
	assert_eq!(anchor_offset(&neighbors, Id(3), 25.0), 65.0);
	assert_eq!(anchor_offset(&neighbors, Id(3), -25.0), 15.0);
	assert_eq!(anchor_offset(&neighbors, Id(3), -100.0), 0.0);
	assert_eq!(anchor_offset(&neighbors, Id(3), 200.0), 140.0);
	assert_eq!(anchor_offset(&[], Id(2), 25.0), 0.0);
}

#[test]
fn channel_rename_invalidates_offscreen_reference_heights() {
	check_channel_rename_heights();
}

#[test]
fn same_id_revision_reset_does_not_reuse_reveal_or_height() {
	let mut message = Message {
		poll: None,
		sticker_items: vec![],
		reactions: Some(vec![]),
		id: Id(1),
		channel: Id(2),
		author: model::User {
			id: Id(3),
			name: "Synthetic".into(),
			avatar: None,
			webhook: false,
			kind: Default::default(),
			discriminator: 0,
			primary_guild: None,
			cosmetics: None,
		},
		content: "||old revealed content||".into(),
		edited: false,
		edited_at: None,
		revision: 0,
		nonce: None,
		reply_to: None,
		kind: 0,
		reply_deleted: false,
		interaction: None,
		forwarded: false,
		unsupported: false,
		extra_content: Default::default(),
		components: vec![],
		application_id: None,
		ephemeral: false,
		flags: 0,
		embeds: vec![],
		attachments: vec![],
		author_nick: None,
		author_roles: vec![],
		mention_roles: vec![],
		mention_everyone: false,
		suppress_notifications: false,
		mentions: Vec::new(),
		embeds_suppressed: false,
	};
	let mut view = TimelineView {
		channel: Some(Id(2)),
		..Default::default()
	};
	view.revealed
		.insert(message.id, Revealed::new(&message, u32::MAX, true));
	view.heights
		.insert(message.id, (layout_key(&message), 4000.0));
	message.content = "||new concealed content||".into();
	let current_key = layout_key(&message);
	assert_ne!(view.heights[&message.id].0, current_key);
	let mut state = State {
		selected: Some(Id(2)),
		revision: 1,
		..Default::default()
	};
	state.timeline.insert(message, false, false).unwrap();
	let context = egui::Context::default();
	// Match dimensions so this specifically exercises content invalidation, not resize.
	let output = context.run_ui(Default::default(), |ui| {
		view.width = ui.available_width();
		view.text_size = egui::TextStyle::Body.resolve(ui.style()).size;
		view.scale = ui.ctx().pixels_per_point();
		view.show(
			ui,
			&mut state,
			&mut None,
			&mut None,
			(
				&mut crate::avatars::Avatars::default(),
				&mut crate::profiles::ProfileSession::default(),
			),
			None,
		);
	});
	output.drop_without_applying_deltas();
	assert!(view.revealed.is_empty());
	assert_eq!(
		view.heights[&Id(1)].0,
		row_key(state.timeline.get(Id(1)).unwrap(), None, None, &state)
	);
	assert!(
		view.heights[&Id(1)].1 < 210.0,
		"A short concealed message must keep a compact row even in an unbounded scroll layout: {}",
		view.heights[&Id(1)].1
	);
}

#[test]
fn successive_deletes_keep_the_floor() {
	let ctx = egui::Context::default();
	crate::design::apply(&ctx);
	let mut state = channel_messages(20, 36);
	let boundary = 86_400_000_u64 << 22;
	let first = boundary - 24;
	state.timeline.clear();
	for (offset, id) in (first..first + 36).enumerate() {
		let mut message = text_message(id);
		message.channel = Id(20);
		message.content = format!("Row {}", offset + 1);
		state.timeline.insert(message, false, false).unwrap();
	}
	state.channels[0].last_message = Some(Id(first + 35));
	state.set_preserve_deleted_messages(true);
	let mut view = TimelineView::default();
	let mut frame = 0u32;
	let mut paint = |view: &mut TimelineView, state: &mut State| {
		frame += 1;
		paint_timeline(&ctx, view, state, frame)
	};
	let mut labels = BTreeMap::new();
	for _ in 0..6 {
		labels = paint(&mut view, &mut state);
	}
	let before: f32 = view.rows.iter().map(|(_, height)| height).sum();
	let before_y = labels["Row 36"];
	for id in first..first + 24 {
		state.timeline.delete(Id(id)).unwrap();
		state.revision += 1;
		labels = paint(&mut view, &mut state);
	}
	for _ in 0..4 {
		labels = paint(&mut view, &mut state);
	}
	let after: f32 = view.rows.iter().map(|(_, height)| height).sum();
	let end_y = labels["Row 36"];
	assert!(
		view.following && (after - before).abs() < 48.0 && (end_y - before_y).abs() < 24.0,
		"successive deletes moved the floor: y={before_y:.1}->{end_y:.1} content={before:.1}->{after:.1} offset={:.1}",
		view.scroll_offset
	);
	let previous = state.timeline.get_display(Id(boundary - 1)).unwrap();
	let successor = state.timeline.get_display(Id(boundary)).unwrap();
	assert!(!grouped(Some(previous), successor, None));
	view.following = false;
	view.jump = false;
	view.anchor = Some((Id(first), 0.0));
	view.revision = u64::MAX;
	for _ in 0..4 {
		labels = paint(&mut view, &mut state);
	}
	let first_day = labels
		.keys()
		.filter(|text| text.contains("January 1,"))
		.count();
	assert_eq!(first_day, 1, "successive deletes opened extra day headers");
	view.anchor = Some((Id(boundary), 0.0));
	view.revision = u64::MAX;
	for _ in 0..4 {
		labels = paint(&mut view, &mut state);
	}
	let next_day = labels
		.keys()
		.filter(|text| text.contains("January 2,"))
		.count();
	assert_eq!(next_day, 1, "successive deletes lost the day boundary");
}

/// Idle frames at the live edge. A moving label is a bounce the reader can see.
#[test]
fn idle_bottom_message_positions_stay_put() {
	for (label, count) in [("long", 80_u64), ("short", 4)] {
		let mut state = State {
			selected: Some(Id(20)),
			revision: 1,
			demo: true,
			older_exhausted: true,
			freshness: model::Freshness::Fresh,
			..Default::default()
		};
		for id in 1..=count {
			let mut message = text_message(id);
			message.content = format!(
				"Row {id}: https://example.com/{id} synthetic idle bottom {}",
				"extra wrapping text ".repeat(if id % 3 == 0 { 10 } else { 0 })
			);
			state.timeline.insert(message, false, false).unwrap();
		}
		let ctx = egui::Context::default();
		crate::design::apply(&ctx);
		let mut view = TimelineView::default();
		let mut avatars = crate::avatars::Avatars::default();
		let mut prev: Option<BTreeMap<String, f32>> = None;
		let mut moves = Vec::new();
		for frame in 0..24 {
			if frame == 16 {
				state.revision += 1;
			}
			let pointer = if frame >= 20 {
				vec![egui::Event::PointerMoved(egui::pos2(180.0, 520.0))]
			} else {
				vec![]
			};
			let output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(900.0, 600.0),
					)),
					time: Some(f64::from(frame) / 60.0),
					focused: true,
					events: pointer,
					..Default::default()
				},
				|ui| {
					view.show(
						ui,
						&mut state,
						&mut None,
						&mut None,
						(
							&mut avatars,
							&mut crate::profiles::ProfileSession::default(),
						),
						None,
					);
				},
			);
			let labels = label_ys(&output);
			let delay = output
				.viewport_output
				.values()
				.next()
				.map(|viewport| viewport.repaint_delay.as_secs_f32());
			if let Some(prev) = &prev {
				let mut worst = 0.0_f32;
				let mut sample = String::new();
				for (text, y) in &labels {
					if !text.starts_with("Row ") || !(80.0..560.0).contains(y) {
						continue;
					}
					if let Some(before) = prev.get(text) {
						let delta = (y - before).abs();
						if delta > worst {
							worst = delta;
							sample = format!("{text} {before:.2}->{y:.2}");
						}
					}
				}
				if worst > 0.5 {
					moves.push(format!(
						"f{frame} {worst:.2}px {sample} off={:.2} follow={} jump={} reflow={} consec={} repaint={delay:?}",
						view.scroll_offset,
						view.following,
						view.jump,
						view.reflow_frames,
						view.consecutive_reflows,
					));
				}
			}
			prev = Some(labels);
			output.drop_without_applying_deltas();
		}
		println!("{label} moves={}", moves.len());
		for line in &moves {
			println!("  {line}");
		}
		let late: Vec<_> = moves
			.iter()
			.filter(|line| {
				let frame: i32 = line
					.trim_start_matches('f')
					.split_whitespace()
					.next()
					.unwrap_or("0")
					.parse()
					.unwrap_or(0);
				frame >= 1
			})
			.cloned()
			.collect();
		assert!(late.is_empty(), "{label} moved after settle: {late:?}");
	}
}

#[test]
fn arriving_messages_keep_the_live_edge_in_view() {
	let ctx = egui::Context::default();
	crate::design::apply(&ctx);
	let mut state = channel_messages(21, 48);
	let mut view = TimelineView::default();
	let mut settled = None;
	for frame in 0..8 {
		settled = newest_y(&paint_timeline(&ctx, &mut view, &mut state, frame), 48);
	}
	let settled = settled.expect("settled tail");
	assert!(view.following, "settled view follows the live edge");
	let arrival = |id: u64| {
		let mut message = text_message(id);
		message.channel = Id(21);
		message.author.id = Id(id);
		message.content = format!("Row {id}\nsecond line\nthird line");
		message
	};
	// Ordinary message: appended to the shared timeline.
	state.timeline.insert(arrival(49), false, false).unwrap();
	state.channels[0].last_message = Some(Id(49));
	state.revision += 1;
	let row_y = |labels: &BTreeMap<String, f32>, id: u64| {
		let prefix = format!("Row {id}\n");
		labels
			.iter()
			.find(|(text, _)| text.starts_with(&prefix))
			.map(|(_, y)| *y)
	};
	let mut seen = Vec::new();
	for frame in 0..8 {
		seen.push(row_y(
			&paint_timeline(&ctx, &mut view, &mut state, 20 + frame),
			49,
		));
	}
	// Three lines of text end where the single settled line ended.
	assert!(
		seen.last()
			.is_some_and(|y| y.is_some_and(|y| y <= settled + 1.0 && y > settled - 80.0)),
		"appended message did not take the live edge: {seen:?} settled={settled} offset={} following={}",
		view.scroll_offset,
		view.following
	);
	// Private command response: merged from the interaction list, with its own footer.
	let mut private = arrival(50);
	private.ephemeral = true;
	private.flags = 64;
	private.interaction = Some(Box::new(model::Interaction {
		user: private.author.clone(),
		command: "ping".into(),
	}));
	state.interactions.ephemeral.push(private);
	state.revision += 1;
	let mut seen = Vec::new();
	for frame in 0..8 {
		seen.push(row_y(
			&paint_timeline(&ctx, &mut view, &mut state, 40 + frame),
			50,
		));
	}
	assert!(
		seen.last()
			.is_some_and(|y| y.is_some_and(|y| y < settled && y > settled - 120.0)),
		"private response did not take the live edge: {seen:?} settled={settled} offset={} following={}",
		view.scroll_offset,
		view.following
	);
	assert!(
		paint_timeline(&ctx, &mut view, &mut state, 60)
			.keys()
			.any(|text| text.starts_with("Only you can see this")),
		"private footer missing"
	);

	{
		let ctx = egui::Context::default();
		crate::design::apply(&ctx);
		let mut state = channel_messages(21, 48);
		let mut view = TimelineView::default();
		let mut settled = None;
		for frame in 0..6 {
			settled = newest_y(&paint_timeline(&ctx, &mut view, &mut state, frame), 48);
		}
		let settled = settled.expect("settled tail");

		view.heights.retain(|id, _| id.0 <= 12);
		view.following = false;
		view.target_browsing = true;
		view.jump = false;
		view.anchor = Some((Id(1), 0.0));
		view.revision = u64::MAX;
		paint_timeline(&ctx, &mut view, &mut state, 20);
		view.follow_latest(&state);
		let mut ack = Vec::new();
		for frame in 0..6 {
			ack.push(newest_y(
				&paint_timeline(&ctx, &mut view, &mut state, 30 + frame),
				48,
			));
		}
		assert!(
			ack.iter()
				.all(|y| y.is_some_and(|y| (y - settled).abs() < 1.0)),
			"ack to the bottom walked the tail {ack:?}, settled at {settled}"
		);

		let mut loading = channel_messages(22, 48);
		loading.timeline.clear();
		loading.freshness = model::Freshness::Loading;
		loading.history_pending = true;
		loading.older_exhausted = false;
		loading.channels[0].last_message = None;
		let mut opened = TimelineView::default();
		paint_timeline(&ctx, &mut opened, &mut loading, 60);
		for id in 1..=48 {
			let mut message = text_message(id);
			message.channel = Id(22);
			message.content = format!("Row {id}");
			loading.timeline.insert(message, false, false).unwrap();
		}
		loading.freshness = model::Freshness::Fresh;
		loading.history_pending = false;
		loading.older_exhausted = true;
		loading.channels[0].last_message = Some(Id(48));
		loading.revision += 1;
		let mut arrived = Vec::new();
		for frame in 0..6 {
			arrived.push(newest_y(
				&paint_timeline(&ctx, &mut opened, &mut loading, 70 + frame),
				48,
			));
		}
		assert!(
			arrived
				.iter()
				.all(|y| y.is_some_and(|y| (y - settled).abs() < 1.0)),
			"opening onto loaded history walked the tail {arrived:?}, settled at {settled}"
		);
	}
}
