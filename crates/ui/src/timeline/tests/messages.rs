use super::helpers::*;
use super::*;

#[test]
fn embed_lightbox_reuses_viewer_and_closes_when_source_is_hidden_or_changed() {
	for reason in [0, 1, 2] {
		let mut state = loading_unread_channel(false);
		state.freshness = model::Freshness::Fresh;
		state.history_pending = false;
		let mut message = text_message(20);
		let media = model::EmbedMedia {
			url: Some("https://cdn.discordapp.com/attachments/1/2/synthetic.png".into()),
			width: 100,
			height: 100,
			..Default::default()
		};
		message.embeds.push(model::Embed {
			kind: "image".into(),
			image: Some(media.clone()),
			description: (reason == 2).then(|| "||Synthetic spoiler||".into()),
			..Default::default()
		});
		message.attachments.push(model::Attachment {
			id: Id(10),
			filename: "Synthetic image.png".into(),
			description: None,
			content_type: Some("image/png".into()),
			size: 64,
			media: media.clone(),
			spoiler: false,
			duration_ms: None,
			waveform: vec![],
		});
		state
			.timeline
			.insert(message.clone(), false, false)
			.unwrap();
		let ctx = egui::Context::default();
		let mut view = TimelineView::default();
		for _ in 0..4 {
			banner_frame(&ctx, &mut view, &mut state, vec![], false);
		}
		if reason == 2 {
			view.download.view_embed(message.id, &media);
			banner_frame(&ctx, &mut view, &mut state, vec![], false);
			assert!(view.embed_viewing.is_none(), "unrevealed media never opens");
			view.revealed
				.insert(message.id, Revealed::new(&message, 0, true));
		}
		view.download.view_embed(message.id, &media);
		banner_frame(&ctx, &mut view, &mut state, vec![], false);
		assert!(view.embed_viewing.is_some());
		assert!(view.opening.is_none() && view.download.request.is_none());
		view.pending_viewer = Some((message.id, Id(10)));
		banner_frame(&ctx, &mut view, &mut state, vec![], false);
		assert!(
			view.embed_viewing.is_none() && view.viewing == Some((message.id, Id(10))),
			"an ordinary attachment replaces the embed viewer"
		);
		view.download.view_embed(message.id, &media);
		banner_frame(&ctx, &mut view, &mut state, vec![], false);
		assert!(
			view.embed_viewing.is_some() && view.viewing.is_none(),
			"an embed replaces the ordinary attachment viewer"
		);
		if reason == 1 {
			message.embeds_suppressed = true;
		} else {
			message.content.push_str(" edited");
		}
		state.timeline.insert(message.clone(), true, false).unwrap();
		banner_frame(&ctx, &mut view, &mut state, vec![], false);
		assert!(view.embed_viewing.is_none());
		if reason == 2 {
			view.download.view_embed(message.id, &media);
			banner_frame(&ctx, &mut view, &mut state, vec![], false);
			assert!(
				view.embed_viewing.is_none(),
				"editing invalidates previous spoiler permission"
			);
		} else if reason == 0 {
			view.download.view_embed(message.id, &media);
			banner_frame(&ctx, &mut view, &mut state, vec![], false);
			assert!(view.embed_viewing.is_some());
			let mut channel = state.channels[0].clone();
			channel.id = Id(21);
			state.channels.push(channel);
			state.selected = Some(Id(21));
			banner_frame(&ctx, &mut view, &mut state, vec![], false);
			assert!(
				view.embed_viewing.is_none(),
				"channel navigation resets the embed viewer"
			);
		}
	}
}

/// Verify message attachments render above rich embeds and link previews.
#[test]
fn attachments_render_above_embeds_and_link_previews() {
	let mut state = loading_unread_channel(false);
	state.freshness = model::Freshness::Fresh;
	state.history_pending = false;
	let id = ((1_788_998_580_000u64 - 1_420_070_400_000) << 22) | 1;
	let mut message = text_message(id);
	message.content = "Message with attachment and embed".into();
	message.attachments.push(model::Attachment {
		id: Id(10),
		filename: "synthetic-upload.txt".into(),
		description: None,
		content_type: Some("text/plain".into()),
		size: 128,
		media: model::EmbedMedia::default(),
		spoiler: false,
		duration_ms: None,
		waveform: vec![],
	});
	message.embeds.push(model::Embed {
		kind: "rich".into(),
		title: Some("Synthetic link preview card".into()),
		description: Some("Card description text".into()),
		..Default::default()
	});
	state.channels[0].last_message = Some(Id(id));
	state.timeline.insert(message, false, false).unwrap();
	let ctx = egui::Context::default();
	let mut view = TimelineView::default();
	let labels = banner_frame(&ctx, &mut view, &mut state, vec![], false);
	let attachment_rect = labels
		.iter()
		.find(|(text, _)| text == "synthetic-upload.txt")
		.expect("attachment filename rendered")
		.1;
	let embed_rect = labels
		.iter()
		.find(|(text, _)| text == "Synthetic link preview card")
		.expect("embed title rendered")
		.1;
	assert!(
		attachment_rect.max.y <= embed_rect.min.y,
		"attachment (bottom: {}) must render above embed card (top: {})",
		attachment_rect.max.y,
		embed_rect.min.y
	);
}

/// Channel switches keep cached video posters while stopping playback.
#[test]
fn channel_switch_keeps_video_posters_but_stops_playback() {
	let ctx = egui::Context::default();
	crate::design::apply(&ctx);
	let mut state = loading_unread_channel(false);
	state.freshness = model::Freshness::Fresh;
	state.history_pending = false;
	let mut view = TimelineView::default();
	banner_frame(&ctx, &mut view, &mut state, vec![], false);
	let attachment = model::Attachment {
		id: Id(10),
		filename: "clip.mp4".into(),
		description: None,
		content_type: Some("video/mp4".into()),
		size: 128,
		media: model::EmbedMedia {
			width: 720,
			height: 1280,
			..Default::default()
		},
		spoiler: false,
		duration_ms: None,
		waveform: vec![],
	};
	assert!(
		view.video
			.accept_poster(&ctx, Id(10), 2, 1, &[1, 1, 1, 255, 2, 2, 2, 255])
	);
	view.video.active = Some((Id(20), Id(20), attachment));
	view.video.state = crate::video::VideoState::Playing;
	let mut channel = state.channels[0].clone();
	channel.id = Id(21);
	state.channels.push(channel);
	state.selected = Some(Id(21));
	banner_frame(&ctx, &mut view, &mut state, vec![], false);
	assert!(
		view.video.posters.contains_key(&Id(10)),
		"posters survive channel switches"
	);
	assert!(view.video.poster_seen.contains(&Id(10)));
	assert!(
		view.video.active.is_none(),
		"switching channels stops playback"
	);
	assert_eq!(view.video.state, crate::video::VideoState::Idle);
	assert!(view.video.command.is_none());
}

#[test]
fn message_menu_allows_authorized_delete_without_exposing_other_authors_edit() {
	fn collect(shape: &egui::Shape, labels: &mut Vec<(String, egui::Rect)>) {
		match shape {
			egui::Shape::Text(text) => {
				labels.push((text.galley.job.text.clone(), text.visual_bounding_rect()))
			}
			egui::Shape::Vec(shapes) => {
				for shape in shapes {
					collect(shape, labels);
				}
			}
			_ => {}
		}
	}
	for (own, can_delete) in [(false, true), (false, false), (true, false)] {
		let ctx = egui::Context::default();
		let message = text_message(1);
		let mut thread_request = None;
		let mut editing = None;
		let mut edit_started = false;
		let mut deleting = None;
		let mut reply = None;
		let mut frame = |events: Vec<egui::Event>| {
			let output = ctx.run_ui(
				egui::RawInput {
					focused: true,
					events,
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(320.0, 400.0),
					)),
					..Default::default()
				},
				|ui| {
					let menu = action_button(ui, crate::icons::Icon::More, "More");
					message_actions(
						egui::Popup::menu(&menu),
						(&message, None, &[], &mut None),
						(own, true, true, can_delete),
						(None, None, &mut reply),
						(&mut editing, &mut edit_started),
						&mut deleting,
						(false, false, &mut None),
						(own, &mut thread_request),
						(false, &mut None),
						None,
					)
				},
			);
			assert!(
				output
					.platform_output
					.commands
					.iter()
					.all(|command| matches!(command, egui::OutputCommand::TextSelectionSettled(_)))
			);
			let mut labels = vec![];
			for shape in &output.shapes {
				collect(&shape.shape, &mut labels);
			}
			output.drop_without_applying_deltas();
			labels
		};
		for _ in 0..2 {
			frame(vec![]);
		}
		// The real More button is keyboard reachable and opens the native egui menu.
		for key in [egui::Key::Tab, egui::Key::Enter] {
			frame(vec![egui::Event::Key {
				key,
				physical_key: None,
				pressed: true,
				repeat: false,
				modifiers: egui::Modifiers::NONE,
			}]);
		}
		frame(vec![]);
		let labels = frame(vec![]);
		assert!(labels.iter().any(|(label, _)| label == "Copy message"));
		assert_eq!(
			labels
				.iter()
				.any(|(label, _)| label.starts_with("Create Thread")),
			own,
			"Create Thread appears only where a thread may be started"
		);
		assert_eq!(labels.iter().any(|(label, _)| label == "Edit message"), own);
		let delete = labels
			.iter()
			.find(|(label, _)| label.starts_with("Delete message"));
		assert_eq!(delete.is_some(), own || can_delete);
		let action = if own {
			labels.iter().find(|(label, _)| label == "Edit message")
		} else {
			delete
		};
		if let Some((_, rect)) = action {
			let pos = rect.center();
			for pressed in [true, false] {
				frame(vec![
					egui::Event::PointerMoved(pos),
					egui::Event::PointerButton {
						pos,
						button: egui::PointerButton::Primary,
						pressed,
						modifiers: egui::Modifiers::NONE,
					},
				]);
			}
		}
		assert!(reply.is_none());
		assert_eq!(
			deleting,
			(!own && can_delete).then_some((message.channel, message.id))
		);
		if own {
			assert_eq!(
				editing,
				Some((message.channel, message.id, message.content.clone()))
			);
			assert!(edit_started);
		} else {
			assert!(editing.is_none() && !edit_started);
		}
	}
}

#[test]
fn inline_reveals_are_independent_of_media_and_reset_on_edit_and_navigation() {
	// Painted text, plus each concealed text spoiler block recorded as "Reveal spoiler".
	fn collect(shape: &egui::Shape, block: egui::Color32, labels: &mut Vec<(String, egui::Rect)>) {
		match shape {
			egui::Shape::Text(text) => {
				labels.push((text.galley.job.text.clone(), text.visual_bounding_rect()))
			}
			egui::Shape::Rect(rect)
				if rect.fill == block
					&& rect.corner_radius == crate::channel_pill::RADIUS.into() =>
			{
				labels.push(("Reveal spoiler".into(), rect.rect));
			}
			egui::Shape::Vec(shapes) => {
				for shape in shapes {
					collect(shape, block, labels);
				}
			}
			_ => {}
		}
	}
	for width in [260.0, 800.0] {
		let ctx = egui::Context::default();
		ctx.set_visuals(if width < 300.0 {
			egui::Visuals::light()
		} else {
			egui::Visuals::dark()
		});
		let mut message = text_message(1);
		message.content =
			"Public before ||secret one|| middle ||secret two|| after `||code literal||`".into();
		message.embeds = vec![model::Embed {
			title: Some("Visible card".into()),
			..Default::default()
		}];
		let mut state = State {
			demo: true,
			selected: Some(message.channel),
			..Default::default()
		};
		state
			.timeline
			.insert(message.clone(), false, false)
			.unwrap();
		let mut view = TimelineView::default();
		let mut images = crate::avatars::Avatars::default();
		let mut render = |view: &mut TimelineView, state: &mut State, events| {
			let mut block = egui::Color32::TRANSPARENT;
			let output = ctx.run_ui(
				egui::RawInput {
					focused: true,
					events,
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(width, 700.0),
					)),
					..Default::default()
				},
				|ui| {
					block = crate::design::palette(ui).muted.gamma_multiply(0.55);
					view.show(
						ui,
						state,
						&mut None,
						&mut None,
						(&mut images, &mut crate::profiles::ProfileSession::default()),
						None,
					)
				},
			);
			assert!(
				output
					.platform_output
					.commands
					.iter()
					.all(|command| matches!(command, egui::OutputCommand::TextSelectionSettled(_)))
			);
			let mut labels = vec![];
			for shape in &output.shapes {
				collect(&shape.shape, block, &mut labels);
			}
			output.drop_without_applying_deltas();
			assert!(view.opening.is_none() && view.channel_reference.is_none());
			labels
		};
		for _ in 0..3 {
			render(&mut view, &mut state, vec![]);
		}
		let labels = render(&mut view, &mut state, vec![]);
		let visible: String = labels.iter().map(|(text, _)| text.as_str()).collect();
		assert!(visible.contains("Public before") && visible.contains("Visible card"));
		assert!(visible.contains("||code literal||"));
		assert!(!visible.contains("secret one") && !visible.contains("secret two"));
		assert_eq!(
			labels
				.iter()
				.filter(|(text, _)| text == "Reveal spoiler")
				.count(),
			2
		);
		let click = |label: &str, labels: &[(String, egui::Rect)]| {
			let pos = labels
				.iter()
				.find(|(text, _)| text == label)
				.unwrap()
				.1
				.center();
			[true, false].map(|pressed| {
				vec![
					egui::Event::PointerMoved(pos),
					egui::Event::PointerButton {
						pos,
						button: egui::PointerButton::Primary,
						pressed,
						modifiers: egui::Modifiers::NONE,
					},
				]
			})
		};
		for events in click("Reveal spoiler", &labels) {
			render(&mut view, &mut state, events);
		}
		let labels = render(&mut view, &mut state, vec![]);
		let visible: String = labels.iter().map(|(text, _)| text.as_str()).collect();
		assert!(visible.contains("secret one") && !visible.contains("secret two"));
		assert_eq!(view.revealed[&message.id].text, 1);
		assert!(!view.revealed[&message.id].media);
		// Like Discord, there is no re-hide control; start the next case concealed.
		view.revealed.clear();
		render(&mut view, &mut state, vec![]);

		// A text reveal cannot grant access to a separately concealed card.
		message.embeds[0].title = Some("||hidden card||".into());
		state.timeline.insert(message.clone(), true, false).unwrap();
		state.revision += 1;
		let labels = render(&mut view, &mut state, vec![]);
		for events in click("Reveal spoiler", &labels) {
			render(&mut view, &mut state, events);
		}
		let labels = render(&mut view, &mut state, vec![]);
		assert!(
			labels
				.iter()
				.any(|(text, _)| text == "Reveal spoiler media")
		);
		assert!(!labels.iter().any(|(text, _)| text.contains("hidden card")));
		for events in click("Reveal spoiler media", &labels) {
			render(&mut view, &mut state, events);
		}
		let labels = render(&mut view, &mut state, vec![]);
		assert!(labels.iter().any(|(text, _)| text.contains("hidden card")));
		assert!(view.revealed[&message.id].media);

		message.content = "Public changed ||new secret||".into();
		state.timeline.insert(message.clone(), true, false).unwrap();
		state.revision += 1;
		let labels = render(&mut view, &mut state, vec![]);
		assert!(
			!labels
				.iter()
				.any(|(text, _)| text.contains("new secret") || text.contains("hidden card"))
		);
		assert!(view.revealed.is_empty());
		let current = labels
			.iter()
			.rfind(|(text, _)| text == "Reveal spoiler")
			.unwrap()
			.1
			.center();
		for pressed in [true, false] {
			render(
				&mut view,
				&mut state,
				vec![
					egui::Event::PointerMoved(current),
					egui::Event::PointerButton {
						pos: current,
						button: egui::PointerButton::Primary,
						pressed,
						modifiers: egui::Modifiers::NONE,
					},
				],
			);
		}
		let labels = render(&mut view, &mut state, vec![]);
		let visible: String = labels.iter().map(|(text, _)| text.as_str()).collect();
		assert!(visible.contains("new secret"));
		assert_eq!(view.revealed[&message.id].text, 1);
		state.selected = Some(Id(30));
		render(&mut view, &mut state, vec![]);
		assert!(view.revealed.is_empty());
	}
}

#[test]
fn the_introduction_leads_once_the_final_older_page_is_loaded() {
	let ctx = egui::Context::default();
	let mut state = test_support::demo_state();
	state.gateway_connected = true;
	state.auth = client_core::auth::AuthState::Authenticated;
	let channel = Id(21);
	let page = |state: &mut State, request, older, ids: std::ops::Range<u64>| {
		let messages = ids
			.rev()
			.map(|id| {
				let mut message = test_support::message(id, channel);
				message.attachments.clear();
				message.embeds.clear();
				message
			})
			.collect();
		state.apply(client_core::Envelope {
			generation: state.generation,
			event: client_core::Event::History {
				channel,
				request,
				older,
				messages,
			},
		});
	};
	let Some(client_core::Command::History { request, .. }) = state.select(channel) else {
		panic!("channel history requested");
	};
	page(&mut state, request, false, 1_000_100..1_000_150);
	let mut view = TimelineView::default();
	let show = |view: &mut TimelineView, state: &mut State| {
		for _ in 0..3 {
			ctx.run_ui(
				egui::RawInput {
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
							&mut crate::avatars::Avatars::default(),
							&mut crate::profiles::ProfileSession::default(),
						),
						None,
					);
				},
			)
			.drop_without_applying_deltas();
		}
	};
	show(&mut view, &mut state);
	assert_ne!(view.rows.first().map(|(id, _)| *id), Some(INTRO));
	let Some(client_core::Command::History { request, .. }) = state.older_history() else {
		panic!("older history requested");
	};
	page(&mut state, request, true, 1_000_000..1_000_003);
	// A finished older page leaves `history_before` set; the window is still complete.
	assert!(state.older_exhausted && state.history_before.is_some());
	show(&mut view, &mut state);
	assert_eq!(
		view.rows
			.iter()
			.take(2)
			.map(|(id, _)| *id)
			.collect::<Vec<_>>(),
		vec![INTRO, Id(1_000_000)]
	);
}

#[test]
fn grouping_respects_dates_replies_unread_and_five_minute_gaps() {
	let mut first = text_message(1);
	let mut next = text_message((60_000 << 22) | 1);
	assert_eq!(timestamp(first.id).date().to_string(), "2015-01-01");
	assert!(grouped(Some(&first), &next, None));
	let original_key = layout_key(&next);
	next.author.kind = model::AccountKind::Bot;
	assert!(!grouped(Some(&first), &next, None));
	assert_ne!(original_key, layout_key(&next));
	next.author.kind = model::AccountKind::Human;
	next.edited = true;
	assert!(grouped(Some(&first), &next, None));
	next.edited = false;
	assert!(!grouped(Some(&first), &next, Some(next.id)));
	let idle = State::default();
	assert_ne!(
		row_key(&next, Some(&first), None, &idle),
		row_key(&next, None, None, &idle)
	);
	next.reply_to = Some(first.id);
	assert!(!grouped(Some(&first), &next, None));
	next.reply_to = None;
	next.id = Id(300_000 << 22);
	assert!(!grouped(Some(&first), &next, None));
	first.id = Id(86_340_000 << 22);
	next.id = Id(86_400_000 << 22);
	assert!(!grouped(Some(&first), &next, None));
	assert_eq!(timestamp(Id(u64::MAX)).year(), 2154);
}

#[test]
fn unsupported_message_fallback_only_requests_confirmation() {
	fn button(shape: &egui::Shape) -> Option<egui::Rect> {
		match shape {
			egui::Shape::Text(t) if t.galley.job.text == "Open in Discord" => {
				Some(t.galley.rect.translate(t.pos.to_vec2()))
			}
			egui::Shape::Vec(shapes) => shapes.iter().find_map(button),
			_ => None,
		}
	}
	let mut state = test_support::demo_state();
	let channel = state
		.channels
		.iter()
		.find(|c| Some(c.id) == state.selected)
		.unwrap()
		.clone();
	let mut message = text_message(42);
	message.channel = channel.id;
	message.unsupported = true;
	state.timeline.clear();
	state.timeline.insert(message, false, false).unwrap();
	for allowed in [true, false] {
		if !allowed {
			state.channels.clear();
		}
		let ctx = egui::Context::default();
		let mut view = TimelineView::default();
		let mut avatars = crate::avatars::Avatars::default();
		let mut render = |view: &mut TimelineView, events| {
			let output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(360.0, 600.0),
					)),
					events,
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
					)
				},
			);
			assert!(
				output
					.platform_output
					.commands
					.iter()
					.all(|command| matches!(command, egui::OutputCommand::TextSelectionSettled(_)))
			);
			let rect = output.shapes.iter().find_map(|s| button(&s.shape));
			output.drop_without_applying_deltas();
			rect
		};
		for _ in 0..3 {
			render(&mut view, vec![]);
		}
		let point = render(&mut view, vec![])
			.expect("Unsupported message has a fallback")
			.center();
		assert!(view.opening.is_none());
		for pressed in [true, false] {
			render(
				&mut view,
				vec![
					egui::Event::PointerMoved(point),
					egui::Event::PointerButton {
						pos: point,
						button: egui::PointerButton::Primary,
						pressed,
						modifiers: egui::Modifiers::NONE,
					},
				],
			);
		}
		assert_eq!(
			view.browser_opening,
			if allowed {
				discord_url(&channel, Some(Id(42)))
			} else {
				None
			}
		);
	}
}

#[test]
fn extra_content_markers_update_layout_and_keep_supported_text() {
	fn collect(shape: &egui::Shape, texts: &mut Vec<(String, egui::Rect)>) {
		match shape {
			egui::Shape::Text(t) => texts.push((
				t.galley.job.text.clone(),
				t.galley.rect.translate(t.pos.to_vec2()),
			)),
			egui::Shape::Vec(shapes) => {
				for shape in shapes {
					collect(shape, texts);
				}
			}
			_ => {}
		}
	}
	let mut state = test_support::demo_state();
	state.read_state.reset();
	let channel = state
		.channels
		.iter()
		.find(|c| Some(c.id) == state.selected)
		.unwrap()
		.clone();
	let mut message = text_message(42);
	message.channel = channel.id;
	message.content = "Supported text remains".into();
	let plain_key = layout_key(&message);
	let ctx = egui::Context::default();
	let mut view = TimelineView::default();
	let mut avatars = crate::avatars::Avatars::default();
	let mut render = |view: &mut TimelineView, state: &mut State, events| {
		let output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(egui::Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(380.0, 650.0),
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
			output.platform_output.commands.is_empty(),
			"Markers only offer explicit external confirmation"
		);
		let mut texts = vec![];
		for shape in &output.shapes {
			collect(&shape.shape, &mut texts);
		}
		output.drop_without_applying_deltas();
		texts
	};
	let all = model::ExtraContent {
		poll: true,
		sticker_items: true,
		stickers: true,
		components: true,
		components_v2: true,
	};
	let mut full_height = None;
	for extra in [
		all,
		model::ExtraContent {
			sticker_items: true,
			..Default::default()
		},
		model::ExtraContent {
			stickers: true,
			..Default::default()
		},
		model::ExtraContent {
			components_v2: true,
			..Default::default()
		},
		model::ExtraContent::default(),
	] {
		message.extra_content = extra;
		assert_eq!(layout_key(&message) == plain_key, !extra.any());
		let mut previous = message.clone();
		previous.id = Id(41);
		assert_eq!(grouped(Some(&previous), &message, None), !extra.any());
		state.timeline.clear();
		state
			.timeline
			.insert(message.clone(), false, false)
			.unwrap();
		state.revision += 1;
		for _ in 0..3 {
			render(&mut view, &mut state, vec![]);
		}
		let texts = render(&mut view, &mut state, vec![]);
		assert!(
			texts
				.iter()
				.any(|(text, _)| text.trim_end() == "Supported text remains")
		);
		for (label, present) in [
			("Poll · Preview unavailable", extra.poll),
			(
				"Sticker · Preview unavailable",
				extra.sticker_items || extra.stickers,
			),
			(
				"Components · Preview unavailable",
				extra.components || extra.components_v2,
			),
			("Open in Discord", extra.any()),
		] {
			assert_eq!(
				texts.iter().filter(|(text, _)| text == label).count(),
				usize::from(present),
				"{label}"
			);
		}
		assert!(
			!texts
				.iter()
				.any(|(text, _)| text == "System content · Preview unavailable")
		);
		assert!(view.opening.is_none());
		if extra == all {
			full_height = Some(view.heights[&message.id].1);
			let point = texts
				.iter()
				.find(|(text, _)| text == "Open in Discord")
				.unwrap()
				.1
				.center();
			for pressed in [true, false] {
				render(
					&mut view,
					&mut state,
					vec![
						egui::Event::PointerMoved(point),
						egui::Event::PointerButton {
							pos: point,
							button: egui::PointerButton::Primary,
							pressed,
							modifiers: egui::Modifiers::NONE,
						},
					],
				);
			}
			assert_eq!(
				view.browser_opening,
				discord_url(&channel, Some(message.id))
			);
			view.browser_opening = None;
		}
		if !extra.any() {
			assert!(
				view.heights[&message.id].1 < full_height.unwrap(),
				"Removing marker-only metadata must shrink the row"
			);
		}
	}
}

#[test]
fn quick_reactions_shift_actions_and_double_click_use_existing_paths() {
	fn texts(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
		match shape {
			egui::Shape::Text(t) => out.push((
				t.galley.job.text.clone(),
				t.galley.rect.translate(t.pos.to_vec2()),
			)),
			egui::Shape::Vec(shapes) => {
				for shape in shapes {
					texts(shape, out);
				}
			}
			_ => {}
		}
	}
	let ctx = egui::Context::default();
	crate::design::apply(&ctx);
	let mut state = test_support::demo_state();
	state.timeline.clear();
	state.read_state.reset();
	let id = Id(60_000 << 22);
	let mut message = text_message(id.0);
	message.content = "Short".into();
	state.user = Some(message.author.clone());
	state.timeline.insert(message, false, false).unwrap();
	let mut view = TimelineView {
		quick_reactions: Some(["🚀", "👍", "❤️"]),
		double_click_reaction: Some("🎉"),
		..Default::default()
	};
	let mut avatars = crate::avatars::Avatars::default();
	let clock = std::cell::Cell::new(0.0);
	let mut render = |view: &mut TimelineView,
	                  state: &mut State,
	                  mut events: Vec<egui::Event>,
	                  modifiers: egui::Modifiers| {
		events.insert(0, egui::Event::ModifiersChanged(modifiers));
		let output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(egui::Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(900.0, 600.0),
				)),
				time: Some(clock.replace(clock.get() + 1.0 / 60.0)),
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
		let mut painted = vec![];
		for shape in &output.shapes {
			texts(&shape.shape, &mut painted);
		}
		let copied: Vec<String> = output
			.platform_output
			.commands
			.iter()
			.filter_map(|command| match command {
				egui::OutputCommand::CopyText(text) => Some(text.clone()),
				_ => None,
			})
			.collect();
		output.drop_without_applying_deltas();
		(painted, copied)
	};
	let click = |pos: egui::Pos2, modifiers: egui::Modifiers| {
		[true, false].map(|pressed| {
			vec![
				egui::Event::PointerMoved(pos),
				egui::Event::PointerButton {
					pos,
					button: egui::PointerButton::Primary,
					pressed,
					modifiers,
				},
			]
		})
	};
	let none = egui::Modifiers::NONE;
	for _ in 0..5 {
		render(&mut view, &mut state, vec![], none);
	}
	let (idle, _) = render(&mut view, &mut state, vec![], none);
	let text = idle.iter().find(|(t, _)| t == "Short").unwrap().1;
	render(
		&mut view,
		&mut state,
		vec![egui::Event::PointerMoved(text.center())],
		none,
	);
	let toolbar = view.toolbar.unwrap().1;
	assert_eq!(toolbar.width(), 240.0);
	// The first quick reaction toggles through the shared reaction path.
	for events in click(toolbar.left_center() + egui::vec2(14.0, 0.0), none) {
		render(&mut view, &mut state, events, none);
	}
	let rocket = model::ReactionEmoji {
		id: None,
		name: Some("🚀".into()),
	};
	assert_eq!(view.reaction.take(), Some((id, Some(rocket.clone()))));
	assert_eq!(view.quick_reaction_used.take(), Some("🚀"));

	// Shift hides quick reactions, adds direct actions and swaps the menu for quick delete.
	let shift = egui::Modifiers::SHIFT;
	render(
		&mut view,
		&mut state,
		vec![egui::Event::PointerMoved(text.center())],
		shift,
	);
	let toolbar = view.toolbar.unwrap().1;
	assert!(toolbar.width() >= 150.0 + 2.0 * 30.0, "{toolbar:?}");
	let mut copied = vec![];
	for events in click(toolbar.right_center() - egui::vec2(74.0, 0.0), shift) {
		copied.extend(render(&mut view, &mut state, events, shift).1);
	}
	assert_eq!(copied, [id.to_string()]);
	for events in click(toolbar.right_center() - egui::vec2(104.0, 0.0), shift) {
		copied.extend(render(&mut view, &mut state, events, shift).1);
	}
	assert_eq!(copied, [id.to_string(), "Short".into()]);
	for events in click(toolbar.right_center() - egui::vec2(44.0, 0.0), shift) {
		render(&mut view, &mut state, events, shift);
	}
	assert_eq!(view.quick_delete.take(), Some((Id(20), id)));
	render(&mut view, &mut state, vec![egui::Event::PointerGone], none);

	// Double-clicking text selects a word; beside it the independent preset toggles.
	for (pos, reacts) in [
		(text.center(), false),
		(egui::pos2(450.0, text.center().y), true),
	] {
		// Let the multi-click window lapse so this pair counts as a double click.
		clock.set(clock.get() + 2.0);
		for _ in 0..2 {
			for events in click(pos, none) {
				render(&mut view, &mut state, events, none);
			}
		}
		assert_eq!(
			view.reaction.take(),
			reacts.then(|| (
				id,
				Some(model::ReactionEmoji {
					id: None,
					name: Some("🎉".into())
				})
			)),
			"{pos:?}"
		);
		render(&mut view, &mut state, vec![egui::Event::PointerGone], none);
	}
}

#[test]
fn deleted_only_timeline_keeps_content_without_service_actions() {
	fn texts(shape: &egui::Shape, out: &mut Vec<String>) {
		match shape {
			egui::Shape::Text(text) => out.push(text.galley.job.text.clone()),
			egui::Shape::Vec(shapes) => {
				for shape in shapes {
					texts(shape, out);
				}
			}
			_ => {}
		}
	}
	for (width, dark) in [(240.0, false), (600.0, true)] {
		let mut state = test_support::demo_state();
		state.read_state.reset();
		state.timeline.clear();
		let mut message = text_message(42);
		message.channel = state.selected.unwrap();
		message.author.name = "Deleted synthetic author".into();
		message.content = "||Deleted synthetic body||".into();
		state
			.timeline
			.insert(message.clone(), false, false)
			.unwrap();
		let ctx = egui::Context::default();
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
					.all(|command| matches!(command, egui::OutputCommand::TextSelectionSettled(_)))
			);
			let mut labels = vec![];
			for shape in &output.shapes {
				texts(&shape.shape, &mut labels);
			}
			output.drop_without_applying_deltas();
			labels
		};
		for _ in 0..3 {
			render(&mut view, &mut state);
		}
		view.revealed
			.insert(message.id, Revealed::new(&message, u32::MAX, true));
		view.viewing = Some((message.id, Id(9)));
		view.toolbar = Some((message.id, egui::Rect::EVERYTHING));
		state.set_preserve_deleted_messages(true);
		state.timeline.delete(message.id).unwrap();
		state.revision += 1;
		for _ in 0..3 {
			render(&mut view, &mut state);
		}
		let labels = render(&mut view, &mut state);
		assert!(state.timeline.is_empty());
		assert_eq!(state.timeline.row_count(), 1);
		assert!(state.timeline.get_display(message.id).is_some());
		assert!(!view.rows.is_empty());
		assert!(
			labels
				.iter()
				.any(|label| label.contains("Deleted synthetic author"))
		);
		assert!(
			labels
				.iter()
				.any(|label| label.contains("Deleted synthetic body"))
		);
		for text in ["Reply", "Open in Discord"] {
			assert!(
				!labels.iter().any(|label| label.contains(text)),
				"Deleted row exposed service action {text}"
			);
		}
		assert!(view.reaction.is_none());
		let selected = state.channel(state.selected.unwrap()).unwrap();
		assert_ne!(selected.last_message, Some(message.id));
		assert_eq!(view.mark_read, selected.last_message);
	}
}

#[test]
fn embed_cards_conceal_spoilers_and_invalidate_reveals_on_embed_only_edits() {
	fn painted_text(shape: &egui::Shape, text: &mut String) {
		match shape {
			egui::Shape::Text(value) => text.push_str(&value.galley.job.text),
			egui::Shape::Vec(shapes) => {
				for shape in shapes {
					painted_text(shape, text);
				}
			}
			_ => {}
		}
	}
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
		content: "Ordinary text".into(),
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
		attachments: vec![],
		author_nick: None,
		author_roles: vec![],
		mention_roles: vec![],
		mention_everyone: false,
		suppress_notifications: false,
		mentions: Vec::new(),
		embeds_suppressed: false,
		embeds: vec![model::Embed {
			kind: "rich".into(),
			title: Some("||Hidden title||".into()),
			description: Some("Embed description".into()),
			image: Some(model::EmbedMedia {
				url: Some("https://example.com/image.png".into()),
				width: 320,
				height: 120,
				..Default::default()
			}),
			..Default::default()
		}],
	};
	// A card near the viewport bottom retains its natural height.
	let ctx = egui::Context::default();
	let mut output = ctx.run_ui(
		egui::RawInput {
			screen_rect: Some(egui::Rect::from_min_size(
				egui::Pos2::ZERO,
				egui::vec2(480.0, 80.0),
			)),
			..Default::default()
		},
		|ui| {
			let mut profile = crate::profiles::ProfileSession::default();
			let _ = crate::embeds::show(
				ui,
				&message,
				&mut FormatCache::default(),
				&mut crate::avatars::Avatars::default(),
				&mut None,
				&mut crate::attachments::DownloadUi::default(),
				&mut profile,
				&State {
					demo: true,
					..Default::default()
				},
				crate::design::MessageCardSurface::Conversation,
				false,
			);
			assert!(
				ui.min_rect().height() > 120.0,
				"card must fit its full image plus text even in an 80 pt viewport"
			);
		},
	);
	output.textures_delta.clear();
	let mut state = State {
		demo: true,
		selected: Some(Id(2)),
		..Default::default()
	};
	state
		.timeline
		.insert(message.clone(), false, false)
		.unwrap();
	let mut view = TimelineView {
		channel: state.selected,
		..Default::default()
	};
	let mut images = crate::avatars::Avatars::default();
	let ctx = egui::Context::default();
	let render =
		|view: &mut TimelineView, state: &mut State, images: &mut crate::avatars::Avatars| {
			let output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(900.0, 900.0),
					)),
					..Default::default()
				},
				|ui| {
					view.show(
						ui,
						state,
						&mut None,
						&mut None,
						(images, &mut crate::profiles::ProfileSession::default()),
						None,
					)
				},
			);
			assert!(
				output.platform_output.commands.is_empty(),
				"Rendering must not open external links"
			);
			let mut text = String::new();
			for shape in &output.shapes {
				painted_text(&shape.shape, &mut text);
			}
			output.drop_without_applying_deltas();
			text
		};
	render(&mut view, &mut state, &mut images);
	let concealed = render(&mut view, &mut state, &mut images);
	assert!(concealed.contains("Reveal spoiler"));
	assert!(!concealed.contains("Hidden title"));
	assert!(!concealed.contains("Embed description"));
	view.revealed
		.insert(message.id, Revealed::new(&message, u32::MAX, true));
	let revealed = render(&mut view, &mut state, &mut images);
	assert!(revealed.contains("Hidden title"));
	assert!(revealed.contains("Embed description"));
	assert!(
		images.take_requests().is_empty(),
		"Demo images never enqueue network requests"
	);
	let previous_key = layout_key(&message);
	message.embeds[0].title = Some("||Changed secret||".into());
	assert_ne!(previous_key, layout_key(&message));
	state.timeline.insert(message.clone(), true, false).unwrap();
	state.revision += 1;
	let changed = render(&mut view, &mut state, &mut images);
	assert!(view.revealed.is_empty());
	assert!(!changed.contains("Changed secret"));
	message.embeds[0].title = Some("Visible title".into());
	message.embeds_suppressed = true;
	state.timeline.insert(message.clone(), true, false).unwrap();
	state.revision += 1;
	let suppressed = render(&mut view, &mut state, &mut images);
	assert!(!suppressed.contains("Visible title"));
	assert!(!suppressed.contains("Embed description"));
	// Attachments are independent of SUPPRESS_EMBEDS, but never of spoiler consent.
	message.embeds.clear();
	message.content.clear();
	message.attachments = vec![model::Attachment {
		duration_ms: None,
		waveform: Vec::new(),
		id: Id(7),
		filename: "SPOILER_hidden.png".into(),
		description: None,
		content_type: Some("image/png".into()),
		size: 100,
		spoiler: true,
		media: model::EmbedMedia {
			url: Some("https://cdn.discordapp.com/attachments/2/7/hidden.png".into()),
			width: 320,
			height: 120,
			..Default::default()
		},
	}];
	assert!(message.attachments[0].is_image());
	state.demo = false; // Only collects image request keys; there is no network worker in this test.
	state.timeline.insert(message.clone(), true, false).unwrap();
	state.revision += 1;
	view.viewing = Some((message.id, Id(7)));
	let hidden = render(&mut view, &mut state, &mut images);
	assert!(!hidden.contains("SPOILER_hidden.png"));
	assert!(view.viewing.is_none());
	assert!(
		images
			.take_requests()
			.iter()
			.all(|key| !key.starts_with("media:")),
		"Hidden attachments must not request media; the visible author avatar is independent"
	);
	view.revealed
		.insert(message.id, Revealed::new(&message, u32::MAX, true));
	view.viewing = Some((message.id, Id(7)));
	render(&mut view, &mut state, &mut images); // Modal sizing pass precedes visible paint.
	let shown = render(&mut view, &mut state, &mut images);
	assert!(shown.contains("SPOILER_hidden.png"));
	assert!(shown.contains("Open in browser"));
	let requests = images.take_requests();
	assert!(!requests.is_empty() && requests.iter().all(|key| key.starts_with("media:")));
	assert!(requests.iter().any(|key| key.contains(":320x120:")));
	let previous_key = layout_key(&message);
	message.attachments[0].description = Some("Changed attachment".into());
	assert_ne!(previous_key, layout_key(&message));
	state.timeline.insert(message, true, false).unwrap();
	state.revision += 1;
	let hidden_again = render(&mut view, &mut state, &mut images);
	assert!(view.viewing.is_none() && view.revealed.is_empty());
	assert!(!hidden_again.contains("SPOILER_hidden.png"));
	assert!(images.take_requests().is_empty());
}
