#[cfg(feature = "demo")]
use super::*;

/// Offline exercise of unread navigation, reply history and returning to the live edge.
#[cfg(feature = "demo")]
pub fn debug_unread_navigation_check(state: &mut State) {
	use client_core::{Command, Envelope, Event, read_state};
	let channel = state.selected.unwrap();
	let template = state.timeline.iter().next().unwrap().clone();
	let message = |id| Message {
		poll: None,
		id: Id(id),
		content: format!("Synthetic message {id}"),
		reply_to: None,
		..template.clone()
	};
	let apply = |state: &mut State, event| {
		state.apply(Envelope {
			generation: state.generation,
			event,
		});
	};
	let page = |state: &mut State, start, end| {
		apply(
			state,
			Event::History {
				channel,
				request: state.request,
				older: state.history_before.is_some(),
				messages: (start..=end).map(&message).collect(),
			},
		);
	};
	let ctx = egui::Context::default();
	let mut view = TimelineView {
		instant_scrolling: true,
		..Default::default()
	};
	let frame = |view: &mut TimelineView, state: &mut State| {
		for _ in 0..5 {
			ctx.run_ui(
				egui::RawInput {
					focused: true,
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(900.0, 600.0),
					)),
					..Default::default()
				},
				|ui| {
					view.show_with_scroll(
						ui,
						state,
						&mut None,
						&mut None,
						(
							&mut crate::avatars::Avatars::default(),
							&mut crate::profiles::ProfileSession::default(),
						),
						None,
						&mut crate::scroll::Session::default(),
					);
				},
			)
			.drop_without_applying_deltas();
		}
	};
	state.history(None);
	page(state, 451, 500);
	state
		.apply_read_state(read_state::Event::Ack {
			channel,
			message: Some(Id(499)),
			manual: true,
			mention_count: Some(0),
			version: None,
		})
		.unwrap();
	frame(&mut view, state);
	let count = state.timeline.row_count();
	assert!(
		state.open_unread().is_none(),
		"Loaded unread must scroll locally"
	);
	assert_eq!(state.search_target, Some(Id(500)));
	assert_eq!(state.timeline.row_count(), count);
	view.browse_away();
	frame(&mut view, state);
	assert_eq!(view.highlighted.map(|(id, _)| id), Some(Id(500)));
	view.follow_latest(state);
	frame(&mut view, state);
	assert_eq!(view.mark_read.take(), Some(Id(500)));
	let Command::MarkRead { request, .. } = state.prepare_mark_read(Id(500)).unwrap() else {
		panic!("Expected read acknowledgement");
	};
	state
		.apply_read_state(read_state::Event::Result {
			channel,
			message: Id(500),
			request,
			result: Ok(()),
		})
		.unwrap();

	// Opening an unloaded reply preserves the read marker and only offers history navigation.
	state.reply = Some(client_core::Reply::to(Id(100)));
	assert!(state.open_reply_target(Id(100)).is_some());
	page(state, 51, 100);
	frame(&mut view, state);
	assert_eq!(state.missed(channel), Some(false));
	assert!(!state.show_missed_banner());
	assert!(state.can_load_newer() && view.present_control.is_some());
	assert!(view.mark_read.is_none());
	let Command::History { after, .. } = state.newer_history().unwrap() else {
		panic!("History");
	};
	assert_eq!(after, Some(Id(100)));
	page(state, 101, 150);
	apply(
		state,
		Event::Delete {
			channel,
			id: Id(150),
		},
	);
	let Command::History { after, .. } = state.newer_history().unwrap() else {
		panic!("History");
	};
	assert_eq!(
		after,
		Some(Id(150)),
		"Deleted trailing rows must not rewind pagination"
	);

	// Older pagination must not disable acknowledgement at the retained live edge.
	state.history(None);
	page(state, 451, 500);
	view.follow_latest(state);
	frame(&mut view, state);
	assert!(state.older_history().is_some());
	page(state, 401, 450);
	apply(state, Event::Message(message(501)));
	frame(&mut view, state);
	assert_eq!(view.mark_read.take(), Some(Id(501)));
	assert_eq!(state.live_edge_latest(), Some(Id(501)));
	apply(state, Event::Message(message(502)));

	// Once the bounded window evicts its newest end, live arrivals cannot bridge the gap.
	for end in (50..=400).rev().step_by(50) {
		assert!(state.older_history().is_some());
		page(state, end - 49, end);
	}
	assert!(state.history_targeted);
	assert!(state.live_edge_latest().is_none());
	apply(
		state,
		Event::Delete {
			channel,
			id: Id(100),
		},
	);
	apply(state, Event::Message(message(503)));
	assert!(state.timeline.get(Id(503)).is_none());
	view.follow_latest(state);
	assert!(
		view.latest,
		"Sending from detached history must request the present page"
	);
	assert_eq!(state.missed(channel), Some(true));
	assert_eq!(
		centered_offset(&[(Id(1), 2000.0)], Id(1), 600.0, 2000.0),
		0.0
	);
}
/// Offline check that the gesture uses its saved preset instead of recommendations.
#[cfg(all(debug_assertions, feature = "demo"))]
impl crate::MessagingUi {
	pub fn debug_double_click_reaction_check(mut state: State, mut message: Message) {
		state.timeline.clear();
		state.read_state.reset();
		state.selected = Some(message.channel);
		message.content = "preset-check".into();
		message.attachments.clear();
		message.embeds.clear();
		message.reactions = Some(vec![]);
		let id = message.id;
		state.user = Some(message.author.clone());
		state.timeline.insert(message, false, false).unwrap();
		let ctx = egui::Context::default();
		crate::design::apply(&ctx);
		let preferences = model::ReadingPreferences {
			double_click_reaction_enabled: true,
			double_click_reaction: 3,
			..Default::default()
		};
		let mut view = TimelineView {
			quick_reactions: Some(["🚀", "👍", "😂"]),
			double_click_reaction: Some(preferences.double_click_emoji()),
			..Default::default()
		};
		let mut avatars = crate::avatars::Avatars::default();
		let mut time = 0.0;
		let mut scroll = crate::scroll::Session::default();
		let modifiers = std::cell::Cell::new(egui::Modifiers::NONE);
		let labels = std::cell::RefCell::new(Vec::<String>::new());
		let menu_id = egui::Id::unique(("message-menu", state.generation, id));
		let mut render = |view: &mut TimelineView, mut events: Vec<egui::Event>| {
			events.insert(0, egui::Event::ModifiersChanged(modifiers.get()));
			time += 1.0 / 60.0;
			let output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(900.0, 600.0),
					)),
					time: Some(time),
					events,
					..Default::default()
				},
				|ui| {
					view.show_with_scroll(
						ui,
						&mut state,
						&mut None,
						&mut None,
						(
							&mut avatars,
							&mut crate::profiles::ProfileSession::default(),
						),
						None,
						&mut scroll,
					);
				},
			);
			let y = output.shapes.iter().find_map(|shape| match &shape.shape {
				egui::Shape::Text(text) if text.galley.text() == "preset-check" => {
					Some(text.pos.y + text.galley.size().y * 0.5)
				}
				_ => None,
			});
			*labels.borrow_mut() = output
				.shapes
				.iter()
				.filter_map(|shape| match &shape.shape {
					egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
					_ => None,
				})
				.collect();
			output.drop_without_applying_deltas();
			y
		};
		for _ in 0..5 {
			render(&mut view, vec![]);
		}
		let pos = egui::pos2(450.0, render(&mut view, vec![]).expect("message laid out"));
		assert!(!model::ReadingPreferences::default().double_click_reaction_enabled);
		for enabled in [false, true] {
			view.double_click_reaction = enabled.then_some(preferences.double_click_emoji());
			for _ in 0..30 {
				render(&mut view, vec![]);
			}
			for _ in 0..2 {
				for pressed in [true, false] {
					render(
						&mut view,
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
			}
			if !enabled {
				assert!(
					view.reaction.is_none(),
					"disabled double-click must not react"
				);
			}
		}
		assert_eq!(
			view.reaction,
			Some((
				id,
				Some(model::ReactionEmoji {
					id: None,
					name: Some("🎉".into())
				})
			))
		);
		modifiers.set(egui::Modifiers::SHIFT);
		render(&mut view, vec![egui::Event::PointerMoved(pos)]);
		let more = view.toolbar.expect("Shift toolbar").1.right_center() - egui::vec2(14.0, 0.0);
		render(&mut view, vec![egui::Event::PointerMoved(more)]);
		render(&mut view, vec![]);
		assert!(
			labels
				.borrow()
				.iter()
				.any(|label| label == &crate::i18n::translate("profiles-show-more")),
			"action label appears without waiting for the tooltip delay: {:?} {:?}",
			labels.borrow(),
			view.toolbar
		);
		for pressed in [true, false] {
			render(
				&mut view,
				vec![egui::Event::PointerButton {
					pos: more,
					button: egui::PointerButton::Primary,
					pressed,
					modifiers: egui::Modifiers::SHIFT,
				}],
			);
		}
		assert!(
			egui::Popup::is_id_open(&ctx, menu_id),
			"Shift-click opens More"
		);
		render(&mut view, vec![]);
		assert!(
			egui::Popup::is_id_open(&ctx, menu_id),
			"More stays open with Shift held"
		);
		modifiers.set(egui::Modifiers::NONE);
		render(&mut view, vec![]);
		assert!(
			egui::Popup::is_id_open(&ctx, menu_id),
			"More stays open after releasing Shift"
		);
		println!("Immediate message labels and Shift menu checks passed.");
		println!("Double-click uses the saved reaction independently of recommendations");
	}
}
