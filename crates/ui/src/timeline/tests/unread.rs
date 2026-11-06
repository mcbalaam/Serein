use super::helpers::*;
use super::*;

#[test]
fn short_history_loads_older_only_after_explicit_upward_input() {
	let mut state = loading_unread_channel(false);
	state.freshness = model::Freshness::Fresh;
	state.history_pending = false;
	let current_id = ((1_788_998_580_000u64 - 1_420_070_400_000) << 22) | 1;
	let mut current = text_message(current_id);
	current.content = "Existing synthetic message".into();
	state.channels[0].last_message = Some(Id(current_id));
	let older_id = current_id - (86_400_000u64 << 22);
	state
		.apply_read_state(client_core::read_state::Event::Snapshot {
			entries: Some(vec![(Id(20), Some(Id(older_id)), 0)]),
			version: Some(2),
			partial: false,
		})
		.unwrap();
	state.timeline.insert(current, false, false).unwrap();
	assert!(state.can_load_older());
	let ctx = egui::Context::default();
	let mut view = TimelineView::default();
	for _ in 0..5 {
		banner_frame(&ctx, &mut view, &mut state, vec![], false);
		assert!(
			!view.load_older,
			"idle short history must not drain older pages"
		);
	}
	let initial = banner_frame(&ctx, &mut view, &mut state, vec![], false);
	let initial_y = initial
		.iter()
		.find(|(text, _)| text == "Existing synthetic message")
		.unwrap()
		.1
		.top();
	let wheel = |delta| {
		vec![
			egui::Event::PointerMoved(egui::pos2(400.0, 300.0)),
			egui::Event::MouseWheel {
				unit: egui::MouseWheelUnit::Point,
				phase: egui::TouchPhase::Move,
				source: egui::MouseWheelSource::Unknown,
				delta: egui::vec2(0.0, delta),
				modifiers: egui::Modifiers::NONE,
			},
		]
	};
	banner_frame(&ctx, &mut view, &mut state, wheel(-60.0), false);
	assert!(!view.load_older);
	banner_frame(&ctx, &mut view, &mut state, wheel(120.0), false);
	assert!(
		view.load_older,
		"upward scroll must reach history even when rows fit in one view"
	);
	assert!(!view.following);
	assert!(state.older_history().is_some());
	for _ in 0..3 {
		banner_frame(&ctx, &mut view, &mut state, vec![], false);
		assert!(
			!view.following && view.target_browsing,
			"waiting for a page preserves browsing intent"
		);
	}
	banner_frame(&ctx, &mut view, &mut state, wheel(120.0), false);
	assert!(
		!view.load_older,
		"pending history must suppress duplicate requests"
	);
	// End the gesture before the response so buffered wheel motion is not
	// mistaken for history restoration. The unread boundary also stays on
	// the existing message, rather than changing its row's chrome.
	banner_frame(
		&ctx,
		&mut view,
		&mut state,
		vec![egui::Event::MouseWheel {
			unit: egui::MouseWheelUnit::Point,
			phase: egui::TouchPhase::End,
			source: egui::MouseWheelSource::Unknown,
			delta: egui::Vec2::ZERO,
			modifiers: egui::Modifiers::NONE,
		}],
		false,
	);
	assert!(!view.following && view.target_browsing);
	let mut older = text_message(older_id);
	older.content = "Synthetic older text\n\n".repeat(40);
	state.apply(client_core::Envelope {
		generation: state.generation,
		event: client_core::Event::History {
			channel: Id(20),
			request: state.request,
			older: true,
			messages: vec![older],
		},
	});
	assert!(!state.history_pending);
	for _ in 0..4 {
		banner_frame(&ctx, &mut view, &mut state, vec![], false);
		assert!(
			!view.following && view.target_browsing,
			"an arriving older page must not jump to the live edge"
		);
	}
	let restored = banner_frame(&ctx, &mut view, &mut state, vec![], false);
	let restored_y = restored
		.iter()
		.find(|(text, _)| text == "Existing synthetic message")
		.unwrap()
		.1
		.top();
	assert!(
		(initial_y - restored_y).abs() <= 2.0,
		"older history preserves the message position: {initial_y} -> {restored_y}"
	);
	view.follow_latest(&state);
	assert!(
		view.following && !view.target_browsing,
		"explicit latest navigation still resumes following"
	);
}

// Synthetic regressions: no transport or acknowledgement worker is running.

#[test]
fn unread_banner_is_visible_as_soon_as_an_unread_channel_is_joined() {
	let ctx = egui::Context::default();
	for cached_reply in [true, false] {
		let mut state = loading_unread_channel(cached_reply);
		let mut view = TimelineView::default();
		let labels = banner_frame(&ctx, &mut view, &mut state, vec![], false);
		assert!(
			labels.iter().any(|(text, _)| text == "Mark as read"),
			"cached reply {cached_reply} while history is still loading showed {labels:?}"
		);
		assert!(
			view.mark_read.is_none(),
			"the loading frame must not acknowledge the channel"
		);
	}
}

#[test]
fn complete_short_channels_never_flash_banners_but_tall_unread_content_is_preserved() {
	for (count, tall, latest) in [
		(0, false, 20),
		(1, false, 20),
		(1, false, 21),
		(1, true, 20),
	] {
		let mut state = test_support::demo_state();
		state.timeline.clear();
		state.selected = Some(Id(20));
		state.auth = client_core::auth::AuthState::Authenticated;
		state.gateway_connected = true;
		state.freshness = model::Freshness::Fresh;
		state.history_pending = false;
		state.history_targeted = false;
		state.history_before = None;
		state.history_after = None;
		state.older_exhausted = true;
		state.channels = vec![model::Channel {
			id: Id(20),
			guild: None,
			parent_id: None,
			position: 0,
			name: "Synthetic complete channel".into(),
			kind: 1,
			recipients: vec![],
			member_list_id: None,
			tags: None,
			message_count: None,
			icon: None,
			// Empty/short history can retain stale service latest metadata.
			last_message: Some(Id(latest)),
		}];
		state
			.apply_read_state(client_core::read_state::Event::Snapshot {
				entries: Some(vec![(Id(20), None, 0)]),
				version: None,
				partial: false,
			})
			.unwrap();
		if count == 1 {
			let mut message = text_message(20);
			if tall {
				message.content = "Synthetic tall unread row\n\n".repeat(250);
			}
			state.timeline.insert(message, false, false).unwrap();
		}
		let ctx = egui::Context::default();
		let mut view = TimelineView::default();
		for _ in 0..5 {
			let labels = banner_frame(&ctx, &mut view, &mut state, vec![], false);
			if tall {
				// Tall unread content opens at its latest message and is read there.
				assert!(view.following && !view.hold_read_ack);
				assert!(
					view.scroll_offset > 400.0,
					"unread join opened at offset {}",
					view.scroll_offset
				);
				continue;
			}
			if count == 0 {
				for forbidden in [
					"Mark as read",
					"Viewing older messages",
					"Jump to unread",
					"New messages below",
					"Jump to present",
				] {
					assert!(
						!labels.iter().any(|(text, _)| text == forbidden),
						"{count} messages unexpectedly showed {forbidden}"
					);
				}
				assert!(!view.hold_read_ack && view.following);
			} else {
				// Opening a short unread channel reads it; the banner does not stay up.
				assert!(
					!labels.iter().any(|(text, _)| text == "Mark as read"),
					"a short unread channel kept its banner after opening: {labels:?}"
				);
				assert!(view.following && !view.hold_read_ack);
			}
		}
		assert_eq!(view.mark_read, Some(Id(latest)));
		if tall {
			// Scrolling to the live edge of tall unread content acknowledges it and resolves
			// both banners, even when the service latest ID names a deleted message.
			state.channels[0].last_message = Some(Id(21));
			view.mark_read = None;
			view.anchor = Some((Id(20), f32::MAX));
			view.revision = u64::MAX;
			banner_frame(&ctx, &mut view, &mut state, vec![], false);
			let labels = banner_frame(
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
			assert!(view.following && !view.target_browsing);
			assert_eq!(view.mark_read.take(), Some(Id(21)));
			for forbidden in [
				"Mark as read",
				"Next messages",
				"New messages below",
				"Jump to present",
			] {
				assert!(
					!labels.iter().any(|(text, _)| text == forbidden),
					"tall stale channel still showed {forbidden}"
				);
			}
		}
		if count == 1 && !tall && latest == 20 {
			// A read snapshot arriving after a local reply jump must not resume reading.
			state.read_state.reset();
			let mut view = TimelineView::default();
			state.search_target = Some(Id(20));
			banner_frame(&ctx, &mut view, &mut state, vec![], false);
			state
				.apply_read_state(client_core::read_state::Event::Snapshot {
					entries: Some(vec![(Id(20), None, 0)]),
					version: None,
					partial: false,
				})
				.unwrap();
			banner_frame(&ctx, &mut view, &mut state, vec![], false);
			assert!(view.target_browsing && view.mark_read.is_none());
		}
	}
}

#[test]
fn browsing_banner_waits_several_screens_and_click_survives_widget_order_changes() {
	let ctx = egui::Context::default();
	let mut state = test_support::demo_state();
	state.read_state.reset();
	state.timeline.clear();
	state.selected = Some(Id(20));
	state.history_targeted = false;
	state.history_before = None;
	state.history_after = None;
	for id in 1..=200 {
		state
			.timeline
			.insert(text_message(id), false, false)
			.unwrap();
	}
	state.revision += 1;
	let mut view = TimelineView::default();
	for _ in 0..5 {
		banner_frame(&ctx, &mut view, &mut state, vec![], false);
	}
	// Several screens of scrollback pass before the control appears at all.
	for (distance, expected) in [(1_200.0, false), (3_000.0, false), (4_200.0, true)] {
		for _ in 0..5 {
			let offset = view.rows.iter().map(|(_, height)| height).sum::<f32>() - 600.0 - distance;
			let (index, _, top) = visible_range(&view.rows, offset, offset);
			view.following = false;
			view.jump = false;
			view.anchor = Some((view.rows[index].0, offset - top));
			view.revision = u64::MAX;
			banner_frame(&ctx, &mut view, &mut state, vec![], false);
		}
		assert_eq!(view.present_control.is_some(), expected);
	}
	banner_frame(&ctx, &mut view, &mut state, vec![], false);
	let pos = view.present_control.unwrap().center();
	for pressed in [true, false] {
		banner_frame(
			&ctx,
			&mut view,
			&mut state,
			vec![
				egui::Event::PointerMoved(pos),
				egui::Event::PointerButton {
					pos,
					button: egui::PointerButton::Primary,
					pressed,
					modifiers: egui::Modifiers::NONE,
				},
			],
			!pressed,
		);
	}
	assert!(
		view.present_scroll.is_some(),
		"Jump to present lost its click when sibling widget IDs changed"
	);
	// The glide back to the live edge resumes following without a teleport.
	for _ in 0..30 {
		banner_frame(&ctx, &mut view, &mut state, vec![], false);
	}
	assert!(view.present_scroll.is_none() && view.following);
}

#[test]
fn reply_target_browsing_waits_for_success_and_explicit_latest_before_acknowledging() {
	fn collect(shape: &egui::Shape, labels: &mut Vec<(String, egui::Rect)>) {
		match shape {
			egui::Shape::Text(text) => labels.push((
				text.galley.job.text.clone(),
				text.galley.rect.translate(text.pos.to_vec2()),
			)),
			egui::Shape::Vec(shapes) => {
				for shape in shapes {
					collect(shape, labels);
				}
			}
			_ => {}
		}
	}
	for (found, width) in [(true, 900.0), (false, 360.0)] {
		let ctx = egui::Context::default();
		let mut state = State {
			auth: client_core::auth::AuthState::Authenticated,
			gateway_connected: true,
			freshness: model::Freshness::Loading,
			history_pending: true,
			selected: Some(Id(20)),
			search_target: Some(Id(19)),
			channels: vec![model::Channel {
				id: Id(20),
				guild: None,
				parent_id: None,
				position: 0,
				name: "Synthetic reply conversation".into(),
				kind: 1,
				recipients: vec![],
				member_list_id: None,
				tags: None,
				message_count: None,
				icon: None,
				last_message: Some(Id(20)),
			}],
			..Default::default()
		};
		state
			.timeline
			.insert(text_message(20), false, false)
			.unwrap();
		if found {
			state
				.timeline
				.insert(text_message(19), false, false)
				.unwrap();
		}
		let mut view = TimelineView::default();
		let mut avatars = crate::avatars::Avatars::default();
		let mut frame = |view: &mut TimelineView, state: &mut State, events| {
			let output = ctx.run_ui(
				egui::RawInput {
					focused: true,
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(width, 600.0),
					)),
					events,
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
			assert!(
				output
					.platform_output
					.commands
					.iter()
					.all(|command| matches!(command, egui::OutputCommand::TextSelectionSettled(_))),
				"{:?}",
				output.platform_output.commands
			);
			let mut labels = vec![];
			for shape in &output.shapes {
				collect(&shape.shape, &mut labels);
			}
			output.drop_without_applying_deltas();
			labels
		};
		for _ in 0..3 {
			frame(&mut view, &mut state, vec![]);
		}
		assert_eq!(state.search_target, Some(Id(19)));
		assert!(view.mark_read.is_none());
		state.freshness = model::Freshness::Stale;
		assert!(view.highlighted.is_none());
		state.history_pending = false;
		state.status = "Synthetic history failure";
		frame(&mut view, &mut state, vec![]);
		assert_eq!(state.status, "Synthetic history failure");
		assert_eq!(state.search_target, Some(Id(19)));
		assert!(view.mark_read.is_none());
		state.freshness = model::Freshness::Fresh;
		state.revision += 1;
		for _ in 0..3 {
			frame(&mut view, &mut state, vec![]);
		}
		assert!(state.search_target.is_none());
		assert_eq!(view.highlighted.map(|(id, _)| id), found.then_some(Id(19)));
		if let Some((_, until)) = &mut view.highlighted {
			*until = -1.0;
		}
		frame(&mut view, &mut state, vec![]);
		assert!(view.highlighted.is_none());
		assert!(view.target_browsing && !view.following);
		assert!(view.mark_read.is_none());
		if !found {
			assert!(state.status.starts_with("Message was not returned"));
		}
		// The whole page fits onscreen, but only an explicit latest action resumes auto-read.
		frame(&mut view, &mut state, vec![]);
		let pos = view
			.present_control
			.expect("jump to present control")
			.center();
		for pressed in [true, false] {
			frame(
				&mut view,
				&mut state,
				vec![
					egui::Event::PointerMoved(pos),
					egui::Event::PointerButton {
						pos,
						button: egui::PointerButton::Primary,
						pressed,
						modifiers: egui::Modifiers::NONE,
					},
				],
			);
		}
		frame(&mut view, &mut state, vec![]);
		assert!(!view.target_browsing);
		assert_eq!(view.mark_read.take(), Some(Id(20)));
	}
}

#[test]
fn server_and_channel_switches_read_each_unread_conversation_on_open() {
	let ctx = egui::Context::default();
	let mut state = unread_servers();
	let mut view = TimelineView::default();

	assert!(matches!(
		state.select(Id(10)),
		Some(client_core::Command::History { .. })
	));
	deliver_unread(&mut state, Id(10), 101);
	settle_banner(&ctx, &mut view, &mut state);
	expect_read_on_open(&mut view, 101, "channel open");

	assert!(matches!(
		state.select(Id(11)),
		Some(client_core::Command::History { .. })
	));
	deliver_unread(&mut state, Id(11), 201);
	settle_banner(&ctx, &mut view, &mut state);
	expect_read_on_open(&mut view, 201, "channel switch");

	// An untouched return reopens without a request; new activity makes it unread again.
	live_message(&mut state, Id(10), 102);
	assert!(matches!(
		state.select(Id(10)),
		Some(client_core::Command::History { .. })
	));
	deliver_unread(&mut state, Id(10), 103);
	settle_banner(&ctx, &mut view, &mut state);
	expect_read_on_open(&mut view, 103, "channel return");

	assert!(matches!(
		state.select_guild(Id(2)),
		Some(client_core::Command::History {
			channel: Id(20),
			..
		})
	));
	deliver_unread(&mut state, Id(20), 301);
	settle_banner(&ctx, &mut view, &mut state);
	expect_read_on_open(&mut view, 301, "server open");

	live_message(&mut state, Id(10), 104);
	assert!(matches!(
		state.select_guild(Id(1)),
		Some(client_core::Command::History {
			channel: Id(10),
			..
		})
	));
	deliver_unread(&mut state, Id(10), 105);
	settle_banner(&ctx, &mut view, &mut state);
	expect_read_on_open(&mut view, 105, "server return");
}

#[test]
fn opening_an_unread_thread_reads_it() {
	// The synthetic forum post 27 lives in server 10 under forum 26.
	let mut state = test_support::demo_state();
	state.gateway_connected = true;
	state.auth = client_core::auth::AuthState::Authenticated;
	let thread = Id(27);
	let Some(client_core::Command::History { request, .. }) = state.select(thread) else {
		panic!("thread history requested");
	};
	let latest = 1_542_322_755_993_600_000u64;
	state
		.channels
		.iter_mut()
		.find(|c| c.id == thread)
		.unwrap()
		.last_message = Some(Id(latest));
	let messages = [latest, latest - 1]
		.into_iter()
		.map(|id| {
			let mut message = text_message(id);
			message.channel = thread;
			message
		})
		.collect();
	state.apply(client_core::Envelope {
		generation: state.generation,
		event: client_core::Event::History {
			channel: thread,
			request,
			older: false,
			messages,
		},
	});
	state
		.apply_read_state(client_core::read_state::Event::Snapshot {
			entries: Some(vec![(thread, Some(Id(latest - 2)), 0)]),
			version: Some(1),
			partial: false,
		})
		.unwrap();
	assert_eq!(state.unread(thread), Some(true));
	let ctx = egui::Context::default();
	let mut view = TimelineView::default();
	let labels = settle_banner(&ctx, &mut view, &mut state);
	assert!(view.following && !view.hold_read_ack);
	assert_eq!(view.mark_read.take(), Some(Id(latest)));
	assert!(
		!plain(&labels)
			.iter()
			.any(|text| text.contains("new message")),
		"opening the thread left its banner up: {labels:?}"
	);
}

#[test]
fn initial_unread_join_reads_on_arrival() {
	// A marker below the loaded page (10) leaves an unread section above it: the banner's
	// Jump to unread stays the way there. Otherwise opening reads the conversation.
	for (marker, width, dark, kind, older_exhausted) in [
		(Some(Id(10)), 320.0, false, 1, false),
		(Some(Id(10)), 320.0, false, 1, true),
		(None, 900.0, true, 1, true),
		(Some(Id(19)), 900.0, true, 1, false),
	] {
		let mut state = State {
			auth: client_core::auth::AuthState::Authenticated,
			gateway_connected: true,
			freshness: model::Freshness::Fresh,
			older_exhausted,
			selected: Some(Id(20)),
			channels: vec![model::Channel {
				id: Id(20),
				guild: None,
				parent_id: None,
				position: 0,
				name: "Synthetic unread conversation".into(),
				kind,
				recipients: vec![],
				member_list_id: None,
				tags: None,
				message_count: None,
				icon: None,
				last_message: Some(Id(20)),
			}],
			..Default::default()
		};
		for id in [19, 20] {
			state
				.timeline
				.insert(text_message(id), false, false)
				.unwrap();
		}
		state
			.apply_read_state(client_core::read_state::Event::Snapshot {
				entries: Some(vec![(Id(20), marker, 0)]),
				version: Some(1),
				partial: false,
			})
			.unwrap();
		let ctx = egui::Context::default();
		ctx.set_visuals(if dark {
			egui::Visuals::dark()
		} else {
			egui::Visuals::light()
		});
		let mut view = TimelineView::default();
		let mut avatars = crate::avatars::Avatars::default();
		let mut frame = |view: &mut TimelineView, state: &mut State, events| {
			let output = ctx.run_ui(
				egui::RawInput {
					focused: true,
					events,
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
					assert!(ui.min_rect().right() <= ui.max_rect().right() + 1.0);
				},
			);
			assert!(
				output
					.platform_output
					.commands
					.iter()
					.all(|command| matches!(command, egui::OutputCommand::TextSelectionSettled(_)))
			);
			output.drop_without_applying_deltas();
		};
		for _ in 0..3 {
			frame(&mut view, &mut state, vec![]);
		}
		let section_loaded = older_exhausted || marker != Some(Id(10));
		assert!(view.following);
		assert_eq!(
			view.hold_read_ack, !section_loaded,
			"kind {kind} marker {marker:?}"
		);
		assert_eq!(
			view.mark_read.take(),
			section_loaded.then_some(Id(20)),
			"kind {kind} marker {marker:?}: opening reads a loaded unread section"
		);
	}
}

#[test]
fn divider_and_dismissed_banner_stay_put_when_messages_arrive_at_the_live_edge() {
	let mut state = State {
		auth: client_core::auth::AuthState::Authenticated,
		gateway_connected: true,
		freshness: model::Freshness::Fresh,
		older_exhausted: true,
		selected: Some(Id(20)),
		channels: vec![model::Channel {
			id: Id(20),
			guild: None,
			parent_id: None,
			position: 0,
			name: "Synthetic unread conversation".into(),
			kind: 1,
			recipients: vec![],
			member_list_id: None,
			tags: None,
			message_count: None,
			icon: None,
			last_message: Some(Id(40)),
		}],
		..Default::default()
	};
	for id in 11..=40 {
		state
			.timeline
			.insert(text_message(id), false, false)
			.unwrap();
	}
	let marker = |state: &mut State, read: u64, version: u64| {
		state
			.apply_read_state(client_core::read_state::Event::Snapshot {
				entries: Some(vec![(Id(20), Some(Id(read)), 0)]),
				version: Some(version),
				partial: false,
			})
			.unwrap();
	};
	marker(&mut state, 32, 1);
	let ctx = egui::Context::default();
	let mut view = TimelineView::default();
	let mut labels = Vec::new();
	for _ in 0..3 {
		labels = banner_frame(&ctx, &mut view, &mut state, vec![], false);
	}
	// Opening at the live edge reads the section: the divider marks it, no banner stays.
	assert_eq!(view.unread_boundary, Some(Id(33)));
	assert!(
		!labels.iter().any(|(text, _)| text.contains("new message")),
		"opening the conversation left its banner up: {labels:?}"
	);
	assert_eq!(view.mark_read.take(), Some(Id(40)));
	marker(&mut state, 40, 2);
	// Someone else posts while the reader watches the latest message.
	state
		.timeline
		.insert(text_message(41), false, false)
		.unwrap();
	state.channels[0].last_message = Some(Id(41));
	for _ in 0..3 {
		labels = banner_frame(&ctx, &mut view, &mut state, vec![], false);
	}
	assert_eq!(
		view.unread_boundary,
		Some(Id(33)),
		"the divider stays at the first message that was unread on arrival"
	);
	assert!(
		!labels.iter().any(|(text, _)| text.contains("new message")),
		"a seen arrival must not revive the dismissed banner: {labels:?}"
	);
	assert_eq!(view.mark_read.take(), Some(Id(41)));
}

#[test]
fn an_unread_section_above_the_page_keeps_its_banner_and_mark_as_read() {
	let ctx = egui::Context::default();
	let mut state = live_unread_channel(8);
	state.older_exhausted = false;
	// A deleted row below the marker keeps only its ID, not a loaded unread section.
	state
		.timeline
		.insert(text_message(5), false, false)
		.unwrap();
	state.timeline.delete(Id(5)).unwrap();
	assert_eq!(state.timeline.row_ids().next(), Some(Id(5)));
	let mut view = TimelineView::default();
	let labels = settle_banner(&ctx, &mut view, &mut state);
	assert!(view.following && view.hold_read_ack && view.mark_read.is_none());
	let pos = labels
		.iter()
		.find(|(text, _)| text == "Mark as read")
		.map(|(_, rect)| rect.center())
		.expect("the held unread banner offers Mark as read");
	click_at(&ctx, &mut view, &mut state, pos);
	assert_eq!(view.mark_channel_read.take(), Some(Id(20)));
	let labels = banner_frame(&ctx, &mut view, &mut state, vec![], false);
	assert!(
		!labels.iter().any(|(text, _)| text == "Mark as read"),
		"Mark as read left the banner up: {labels:?}"
	);
}

#[test]
fn arrivals_at_a_focused_live_edge_are_read_without_a_banner() {
	let ctx = egui::Context::default();
	let mut state = live_unread_channel(40);
	let mut view = TimelineView::default();
	settle_banner(&ctx, &mut view, &mut state);
	assert!(view.following && !view.hold_read_ack);
	for (version, id) in [(2, 41), (3, 42), (4, 43)] {
		if id == 42 {
			// A tall arrival is measured after it lands; the reader still follows it.
			let mut tall = text_message(id);
			tall.content = "Synthetic tall live row\n\n".repeat(30);
			state.timeline.insert(tall, false, false).unwrap();
		}
		arrive(&mut state, id);
		for _ in 0..4 {
			let labels = plain(&banner_frame(&ctx, &mut view, &mut state, vec![], false));
			assert!(
				!labels
					.iter()
					.any(|text| text == "Mark as read" || text.contains("new message")),
				"a watched arrival raised the unread banner: {labels:?}"
			);
			assert!(
				view.following && view.present_control.is_none(),
				"{labels:?}"
			);
		}
		assert_eq!(
			view.mark_read.take(),
			Some(Id(id)),
			"the arrival is acknowledged"
		);
		set_read_marker(&mut state, id, version);
	}
	assert_eq!(
		view.unread_boundary, None,
		"watched arrivals add no divider"
	);
}

#[test]
fn opening_reads_the_section_and_later_arrivals_stay_read_at_the_bottom() {
	let ctx = egui::Context::default();
	let mut state = live_unread_channel(32);
	let mut view = TimelineView::default();
	let labels = plain(&settle_banner(&ctx, &mut view, &mut state));
	// Opening at the live edge reads the section instead of leaving its banner up.
	assert!(
		!labels.iter().any(|text| text.contains("new message")),
		"{labels:?}"
	);
	assert_eq!(view.mark_read.take(), Some(Id(40)));
	// A message arrives before the acknowledgement round trip completes, and another after.
	arrive(&mut state, 41);
	let labels = plain(&settle_banner(&ctx, &mut view, &mut state));
	assert!(
		!labels.iter().any(|text| text == "Mark as read"),
		"an arrival revived the read banner: {labels:?}"
	);
	set_read_marker(&mut state, 41, 2);
	view.mark_read = None;
	for (version, id) in [(3, 42), (4, 43)] {
		arrive(&mut state, id);
		let labels = plain(&settle_banner(&ctx, &mut view, &mut state));
		assert!(
			!labels
				.iter()
				.any(|text| text == "Mark as read" || text.contains("new message")),
			"an arrival at the bottom revived the banner: {labels:?}"
		);
		assert_eq!(view.mark_read.take(), Some(Id(id)));
		set_read_marker(&mut state, id, version);
	}
}

#[test]
fn arrivals_while_scrolled_up_count_only_unseen_messages_and_offer_the_way_down() {
	let ctx = egui::Context::default();
	let mut state = live_unread_channel(32);
	let mut view = TimelineView::default();
	settle_banner(&ctx, &mut view, &mut state);
	// Read down to the live edge: the section is acknowledged and its banner leaves.
	banner_frame(
		&ctx,
		&mut view,
		&mut state,
		vec![
			egui::Event::PointerMoved(egui::pos2(450.0, 300.0)),
			egui::Event::MouseWheel {
				unit: egui::MouseWheelUnit::Point,
				delta: egui::vec2(0.0, -80.0),
				modifiers: egui::Modifiers::NONE,
				phase: egui::TouchPhase::Move,
				source: egui::MouseWheelSource::Unknown,
			},
		],
		false,
	);
	assert_eq!(view.mark_read.take(), Some(Id(40)));
	set_read_marker(&mut state, 40, 2);
	settle_banner(&ctx, &mut view, &mut state);
	// Scroll a screen up, then two messages arrive out of sight.
	let scroll_up = |view: &mut TimelineView, state: &mut State| {
		for _ in 0..4 {
			let total = view.rows.iter().map(|(_, height)| height).sum::<f32>();
			let offset = (total - 600.0 - 400.0).max(0.0);
			let (index, _, top) = visible_range(&view.rows, offset, offset);
			view.following = false;
			view.jump = false;
			view.anchor = Some((view.rows[index].0, offset - top));
			view.revision = u64::MAX;
			banner_frame(&ctx, view, state, vec![], false);
		}
	};
	scroll_up(&mut view, &mut state);
	assert!(!view.following);
	assert!(view.present_control.is_none(), "nothing new below yet");
	arrive(&mut state, 41);
	arrive(&mut state, 42);
	scroll_up(&mut view, &mut state);
	let labels = plain(&banner_frame(&ctx, &mut view, &mut state, vec![], false));
	assert!(
		labels
			.iter()
			.any(|text| text.starts_with("2 new messages since")),
		"the banner counts only the unseen arrivals: {labels:?}"
	);
	assert_eq!(
		view.unread_boundary,
		Some(Id(41)),
		"unseen arrivals start a new section"
	);
	assert!(
		labels.iter().any(|text| text == "2 new messages"),
		"the way down names the unseen arrivals: {labels:?}"
	);
	assert!(view.mark_read.is_none(), "unseen arrivals stay unread");
	let pos = view.present_control.expect("a way back down").center();
	click_at(&ctx, &mut view, &mut state, pos);
	for _ in 0..30 {
		banner_frame(&ctx, &mut view, &mut state, vec![], false);
	}
	assert!(view.following && view.present_control.is_none());
	assert_eq!(
		view.mark_read.take(),
		Some(Id(42)),
		"reaching the bottom reads them"
	);
	set_read_marker(&mut state, 42, 3);
	let labels = plain(&settle_banner(&ctx, &mut view, &mut state));
	assert!(
		!labels
			.iter()
			.any(|text| text == "Mark as read" || text.contains("new message")),
		"reaching the bottom reset the banner: {labels:?}"
	);
}

#[test]
fn auto_read_requires_focused_latest_and_does_not_retry_failed_marker() {
	let mut state = State {
		auth: client_core::auth::AuthState::Authenticated,
		gateway_connected: true,
		freshness: model::Freshness::Fresh,
		selected: Some(Id(20)),
		channels: vec![model::Channel {
			id: Id(20),
			guild: None,
			parent_id: None,
			position: 0,
			name: "Synthetic DM".into(),
			kind: 1,
			recipients: vec![],
			member_list_id: None,
			tags: None,
			message_count: None,
			icon: None,
			last_message: Some(Id(1)),
		}],
		..Default::default()
	};
	state
		.timeline
		.insert(text_message(1), false, false)
		.unwrap();
	let ctx = egui::Context::default();
	let mut view = TimelineView::default();
	let mut avatars = crate::avatars::Avatars::default();
	let mut frame = |view: &mut TimelineView, state: &mut State, focused| {
		ctx.run_ui(
			egui::RawInput {
				focused,
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
	frame(&mut view, &mut state, false);
	assert!(view.mark_read.is_none());
	frame(&mut view, &mut state, true);
	assert_eq!(view.mark_read.take(), Some(Id(1)));
	frame(&mut view, &mut state, true);
	assert!(view.mark_read.is_none());
	state.channels[0].last_message = Some(Id(3));
	state
		.timeline
		.insert(text_message(2), false, false)
		.unwrap();
	state.revision += 1;
	state.history_before = Some(Id(2));
	frame(&mut view, &mut state, true);
	assert!(
		view.mark_read.is_none(),
		"Historical window is not the latest message"
	);
	// The latest page is the live edge; latest metadata beyond it outlived a deletion.
	state.history_before = None;
	state.revision += 1;
	frame(&mut view, &mut state, true);
	assert_eq!(view.mark_read.take(), Some(Id(3)));
}
