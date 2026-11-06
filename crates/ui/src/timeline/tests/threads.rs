use super::helpers::*;
use super::*;

#[test]
fn thread_starter_renders_above_replies_once_history_is_exhausted() {
	let ctx = egui::Context::default();
	crate::design::apply(&ctx);
	let mut state = test_support::chat_demo_state();
	state
		.permissions
		.replace(test_support::permission_snapshot(&state))
		.unwrap();
	let (thread, parent) = state
		.channels
		.iter()
		.find(|c| {
			matches!(c.kind, 10..=12)
				&& c.parent_id
					.and_then(|id| state.channel(id))
					.is_some_and(|p| matches!(p.kind, 0 | 5))
		})
		.map(|c| (c.id, c.parent_id.unwrap()))
		.expect("fixture thread under a text channel");
	state.select(thread);
	for id in [thread.0 + 10, thread.0 + 20] {
		state
			.timeline
			.insert(test_support::message(id, thread), false, false)
			.unwrap();
	}
	state.history_pending = false;
	state.freshness = model::Freshness::Fresh;
	state.older_exhausted = true;
	let Some(client_core::Command::ThreadStarter { request, .. }) = state.request_thread_starter()
	else {
		panic!("thread requests its starter");
	};
	let mut starter = test_support::message(thread.0, parent);
	starter.content = "The message that started it all".into();
	state.apply(client_core::Envelope {
		generation: state.generation,
		event: client_core::Event::ThreadStarter {
			thread,
			request,
			result: Ok(starter),
		},
	});
	let mut view = TimelineView::default();
	let mut avatars = crate::avatars::Avatars::default();
	let mut painted = vec![];
	for _ in 0..4 {
		painted.clear();
		let output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(egui::Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(900.0, 1400.0),
				)),
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
		for shape in &output.shapes {
			painted_labels(&shape.shape, &mut painted);
		}
		output.drop_without_applying_deltas();
	}
	// The thread's introduction leads, then its starter, then the replies.
	assert_eq!(
		view.rows
			.iter()
			.take(2)
			.map(|(id, _)| *id)
			.collect::<Vec<_>>(),
		vec![INTRO, thread]
	);
	for label in [
		"The message that started it all",
		"Thread started from this message",
	] {
		assert!(
			painted.iter().any(|t| t == label),
			"missing {label}: {painted:?}"
		);
	}
}

#[test]
fn an_unloaded_thread_starter_offers_its_thread_card() {
	let ctx = egui::Context::default();
	crate::design::apply(&ctx);
	ctx.enable_accesskit();
	let mut state = test_support::chat_demo_state();
	state
		.permissions
		.replace(test_support::permission_snapshot(&state))
		.unwrap();
	let parent = state
		.channels
		.iter()
		.find(|c| c.id == Id(20))
		.map(|c| c.id)
		.expect("fixture text channel");
	state.select(parent);
	state.timeline.clear();
	// The service flags one message as a thread's starter, but the session never loaded
	// that thread (a private thread the account can view without being a member).
	let thread = Id(1_700_000_000_000_000_000);
	assert!(state.channel(thread).is_none());
	let mut starter = test_support::message(thread.0, parent);
	starter.flags = model::HAS_THREAD;
	starter.content = "The private thread starter".into();
	let starter_id = starter.id;
	let ordinary = test_support::message(thread.0 + 1, parent);
	for message in [starter, ordinary] {
		state.timeline.insert(message, false, false).unwrap();
	}
	state.history_pending = false;
	state.freshness = model::Freshness::Fresh;
	state.older_exhausted = true;
	let mut view = TimelineView::default();
	let mut avatars = crate::avatars::Avatars::default();
	let mut painted = vec![];
	for _ in 0..4 {
		painted.clear();
		let output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(egui::Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(900.0, 1400.0),
				)),
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
		for shape in &output.shapes {
			painted_labels(&shape.shape, &mut painted);
		}
		output.drop_without_applying_deltas();
	}
	// Only the flagged starter offers its thread, and the card is its own click target: the
	// "View thread" button and the generic "Thread" label are both gone.
	assert_eq!(view.thread_preview_request, Some(starter_id));
	assert!(
		!painted.iter().any(|text| text == "View thread ›"),
		"the card itself opens the thread: {painted:?}"
	);
	assert!(
		!painted.iter().any(|text| text == "Thread"),
		"an unknown thread name is left out instead of invented: {painted:?}"
	);

	// The bounded read fills the card: the thread's name and count, then its last author,
	// reply and relative time.
	let mut reply = test_support::message(thread.0 + 2, thread);
	reply.content = "The latest reply in this thread".into();
	let client_core::Command::ThreadPreview {
		thread: requested,
		request,
		..
	} = state
		.request_thread_preview(starter_id)
		.expect("the visible card admits one preview read")
	else {
		panic!("ThreadPreview")
	};
	assert_eq!(requested, thread);
	state.apply(client_core::Envelope {
		generation: state.generation,
		event: client_core::Event::ThreadPreview {
			thread,
			request,
			result: Ok(client_core::thread_previews::Loaded {
				channel: Some(model::Channel {
					id: thread,
					guild: Some(Id(10)),
					parent_id: Some(parent),
					kind: 12,
					position: 0,
					name: "Synthetic private thread".into(),
					recipients: vec![],
					last_message: Some(reply.id),
					member_list_id: None,
					tags: None,
					message_count: Some(3),
					icon: None,
				}),
				last: Some(reply),
			}),
		},
	});
	view.thread_preview_request = None;
	painted.clear();
	let mut accessible = Vec::new();
	for _ in 0..2 {
		let output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(egui::Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(900.0, 1400.0),
				)),
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
		for shape in &output.shapes {
			painted_labels(&shape.shape, &mut painted);
		}
		if let Some(update) = output.platform_output.accesskit_update.as_ref() {
			accessible.extend(
				update
					.nodes
					.iter()
					.filter_map(|(_, node)| node.label().map(str::to_owned)),
			);
		}
		output.drop_without_applying_deltas();
	}
	let count = format!("{} ›", thread_count(3));
	for label in [
		"Synthetic private thread",
		count.as_str(),
		"You (synthetic)",
		"The latest reply in this thread",
	] {
		assert!(
			painted.iter().any(|text| text == label),
			"missing {label}: {painted:?}"
		);
	}
	// Painted text has no accessible node of its own, so the card names every part.
	assert!(
		accessible.iter().any(|label| {
			label.contains("Synthetic private thread")
				&& label.contains(&thread_count(3))
				&& label.contains("You (synthetic): The latest reply in this thread")
		}),
		"the card names its name, count, author and preview: {accessible:?}"
	);
	assert!(
		view.thread_preview_request.is_none(),
		"a trusted preview is not read again: {painted:?}"
	);
}

#[test]
fn a_thread_preview_names_mentions_hides_spoilers_and_falls_back_to_what_a_message_carries() {
	let ctx = egui::Context::default();
	crate::design::apply(&ctx);
	let state = test_support::demo_state();
	let mut formatted = FormatCache::default();
	let mut message = text_message(7);
	message.content = "  first line\nsecond line  ".into();
	let text = |message: &Message, formatted: &mut FormatCache| {
		let mut text = None;
		let output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(egui::Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(800.0, 600.0),
				)),
				..Default::default()
			},
			|ui| {
				text = Some(thread_preview_text(ui, &state, formatted, Id(20), message));
			},
		);
		output.drop_without_applying_deltas();
		text.unwrap()
	};
	assert_eq!(text(&message, &mut formatted), "first line second line");

	// A service row uses its summary, so its card is not blank.
	message.kind = 18;
	message.content = "Synthetic thread".into();
	let preview = text(&message, &mut formatted);
	assert!(preview.contains("started a thread"), "{preview}");
	message.kind = 0;

	// A mention names its author instead of the raw token.
	message.content = "Hi <@9>".into();
	message.mentions = vec![model::User {
		id: Id(9),
		name: "Synthetic".into(),
		avatar: None,
		webhook: false,
		kind: Default::default(),
		discriminator: 0,
		primary_guild: None,
		cosmetics: None,
	}];
	let preview = text(&message, &mut formatted);
	assert!(
		preview.contains("Synthetic") && !preview.contains('<'),
		"{preview}"
	);

	// A card cannot reveal a spoiler, so it only says there is one.
	message.content = "A ||private ending|| Z".into();
	message.mentions.clear();
	assert_eq!(text(&message, &mut formatted), "Spoiler");

	// The concealment can live in an embed instead of the message text.
	message.content.clear();
	message.embeds = vec![model::Embed {
		title: Some("||the ending||".into()),
		..Default::default()
	}];
	assert_eq!(text(&message, &mut formatted), "Spoiler");
	message.embeds.clear();

	message.content.clear();
	message.attachments = vec![model::Attachment {
		id: Id(8),
		filename: "synthetic.png".into(),
		description: None,
		content_type: None,
		size: 1,
		media: Default::default(),
		spoiler: false,
		duration_ms: None,
		waveform: vec![],
	}];
	assert_eq!(text(&message, &mut formatted), "synthetic.png");
}
