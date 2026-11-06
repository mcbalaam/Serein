use super::*;

/// Text actually painted this frame, in painting order.
pub(super) fn painted_labels(shape: &egui::Shape, out: &mut Vec<String>) {
	match shape {
		egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
		egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| painted_labels(s, out)),
		_ => {}
	}
}

pub(super) fn banner_frame(
	ctx: &egui::Context,
	view: &mut TimelineView,
	state: &mut State,
	events: Vec<egui::Event>,
	shift_widget_order: bool,
) -> Vec<(String, egui::Rect)> {
	banner_frame_bounds(ctx, view, state, events, shift_widget_order, false)
}

pub(super) fn banner_frame_bounds(
	ctx: &egui::Context,
	view: &mut TimelineView,
	state: &mut State,
	events: Vec<egui::Event>,
	shift_widget_order: bool,
	actual_glyphs: bool,
) -> Vec<(String, egui::Rect)> {
	banner_frame_bounds_width(
		ctx,
		view,
		state,
		events,
		shift_widget_order,
		actual_glyphs,
		900.0,
	)
}

pub(super) fn banner_frame_bounds_width(
	ctx: &egui::Context,
	view: &mut TimelineView,
	state: &mut State,
	events: Vec<egui::Event>,
	shift_widget_order: bool,
	actual_glyphs: bool,
	width: f32,
) -> Vec<(String, egui::Rect)> {
	fn collect(shape: &egui::Shape, labels: &mut Vec<(String, egui::Rect)>, actual_glyphs: bool) {
		match shape {
			egui::Shape::Text(text) => {
				let mut rect = text.galley.rect.translate(text.pos.to_vec2());
				if actual_glyphs {
					// Wrapped labels include the occupied author space in their galley
					// rectangle; inspect actual first-row glyphs for painted text positions.
					if let Some(row) = text.galley.rows.first()
						&& let Some(glyph) = row.glyphs.first()
					{
						rect.min.x = text.pos.x + row.pos.x + glyph.pos.x;
					}
				}
				labels.push((text.galley.job.text.clone(), rect));
			}
			egui::Shape::Vec(shapes) => {
				for shape in shapes {
					collect(shape, labels, actual_glyphs);
				}
			}
			_ => {}
		}
	}
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
			// A conditional sibling changes auto IDs without moving the bar.
			if shift_widget_order {
				ui.skip_ahead_auto_ids(1);
			}
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
		collect(&shape.shape, &mut labels, actual_glyphs);
	}
	output.drop_without_applying_deltas();
	labels
}

pub(super) fn loading_unread_channel(cached_reply: bool) -> State {
	let mut state = State {
		auth: client_core::auth::AuthState::Authenticated,
		gateway_connected: true,
		freshness: model::Freshness::Loading,
		history_pending: true,
		history_targeted: false,
		history_before: None,
		history_after: None,
		older_exhausted: false,
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
			last_message: Some(Id(20)),
		}],
		..Default::default()
	};
	state
		.apply_read_state(client_core::read_state::Event::Snapshot {
			entries: Some(vec![(Id(20), Some(Id(10)), 0)]),
			version: Some(1),
			partial: false,
		})
		.unwrap();
	if cached_reply {
		let mut message = text_message(20);
		message.reply_to = Some(Id(9));
		message.reactions = Some(vec![model::Reaction {
			emoji: model::ReactionEmoji {
				id: None,
				name: Some("wave".into()),
			},
			count: 1,
			me: false,
			me_burst: false,
		}]);
		state.timeline.insert(message, false, false).unwrap();
	}
	state
}

pub(super) fn text_message(id: u64) -> Message {
	Message {
		poll: None,
		sticker_items: vec![],
		id: Id(id),
		channel: Id(20),
		author: model::User {
			id: Id(2),
			name: "Robin".into(),
			avatar: None,
			webhook: false,
			kind: Default::default(),
			discriminator: 0,
			primary_guild: None,
			cosmetics: None,
		},
		content: "Synthetic text with enough words to wrap in a narrow viewport.".into(),
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
		mentions: vec![],
		reactions: Some(vec![]),
		embeds_suppressed: false,
	}
}

pub(super) fn unread_servers() -> State {
	use model::permissions as p;
	let channels = [(10, 1), (11, 1), (20, 2)]
		.into_iter()
		.map(|(id, guild)| model::Channel {
			id: Id(id),
			guild: Some(Id(guild)),
			parent_id: None,
			position: id as i32,
			name: format!("synthetic-{id}"),
			kind: 0,
			recipients: vec![],
			member_list_id: None,
			tags: None,
			message_count: None,
			icon: None,
			last_message: None,
		})
		.collect::<Vec<_>>();
	let permission_channels = channels
		.iter()
		.filter_map(|channel| {
			channel.guild.map(|guild| p::Channel {
				id: channel.id,
				guild,
				overwrites: Some(vec![]),
			})
		})
		.collect();
	let mut state = State {
		auth: client_core::auth::AuthState::Authenticated,
		gateway_connected: true,
		user: Some(model::User {
			id: Id(999),
			name: "Synthetic".into(),
			avatar: None,
			webhook: false,
			kind: Default::default(),
			discriminator: 0,
			primary_guild: None,
			cosmetics: None,
		}),
		guilds: [1, 2]
			.into_iter()
			.map(|id| model::Guild {
				default_message_notifications: None,
				stickers: None,
				emojis: None,
				id: Id(id),
				name: format!("Server {id}"),
				icon: None,
			})
			.collect(),
		channels,
		..State::default()
	};
	state
		.permissions
		.replace(p::Snapshot {
			guilds: (1..=2)
				.map(|id| p::Guild {
					id: Id(id),
					owner: Some(Id(999)),
					member: Some(p::Member {
						roles: vec![],
						timeout_until: None,
					}),
					roles: Some(vec![p::Role {
						id: Id(id),
						name: String::new(),
						color: 0,
						secondary_color: None,
						tertiary_color: None,
						position: 0,
						hoist: false,
						managed: false,
						bits: p::VIEW_CHANNEL | p::READ_MESSAGE_HISTORY,
					}]),
				})
				.collect(),
			channels: permission_channels,
		})
		.unwrap();
	state
		.apply_read_state(client_core::read_state::Event::Snapshot {
			entries: Some(vec![
				(Id(10), Some(Id(1)), 0),
				(Id(11), Some(Id(1)), 0),
				(Id(20), Some(Id(1)), 0),
			]),
			version: Some(1),
			partial: false,
		})
		.unwrap();
	state
}

pub(super) fn deliver_unread(state: &mut State, channel: Id, latest: u64) {
	assert_eq!(state.selected, Some(channel));
	assert!(state.history_pending);
	let messages = [latest - 1, latest]
		.into_iter()
		.map(|id| {
			let mut message = test_support::message(id, channel);
			message.content = "Synthetic tall unread row\n\n".repeat(40);
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
	assert_eq!(state.freshness, model::Freshness::Fresh);
	assert_eq!(state.unread(channel), Some(true));
}

pub(super) fn settle_banner(
	ctx: &egui::Context,
	view: &mut TimelineView,
	state: &mut State,
) -> Vec<(String, egui::Rect)> {
	let mut labels = vec![];
	for _ in 0..4 {
		labels = banner_frame(ctx, view, state, vec![], false);
	}
	labels
}

pub(super) fn expect_read_on_open(view: &mut TimelineView, latest: u64, step: &str) {
	assert!(
		!view.hold_read_ack && view.following,
		"{step}: hold={} following={}",
		view.hold_read_ack,
		view.following
	);
	assert_eq!(
		view.mark_read.take(),
		Some(Id(latest)),
		"{step} did not acknowledge the conversation it opened"
	);
}

pub(super) fn live_message(state: &mut State, channel: Id, id: u64) {
	state.apply(client_core::Envelope {
		generation: state.generation,
		event: client_core::Event::Message(test_support::message(id, channel)),
	});
}

/// A synthetic DM with messages 11..=40 loaded and a read marker at `read`.
pub(super) fn live_unread_channel(read: u64) -> State {
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
			name: "Synthetic live conversation".into(),
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
	set_read_marker(&mut state, read, 1);
	state
}

pub(super) fn set_read_marker(state: &mut State, read: u64, version: u64) {
	state
		.apply_read_state(client_core::read_state::Event::Snapshot {
			entries: Some(vec![(Id(20), Some(Id(read)), 0)]),
			version: Some(version),
			partial: false,
		})
		.unwrap();
}

pub(super) fn arrive(state: &mut State, id: u64) {
	if state.timeline.get(Id(id)).is_none() {
		state
			.timeline
			.insert(text_message(id), false, false)
			.unwrap();
	}
	state.channels[0].last_message = Some(Id(id));
	state.revision += 1;
}

pub(super) fn plain(labels: &[(String, egui::Rect)]) -> Vec<String> {
	labels
		.iter()
		.map(|(text, _)| text.replace(['\u{2068}', '\u{2069}'], ""))
		.collect()
}

pub(super) fn click_at(
	ctx: &egui::Context,
	view: &mut TimelineView,
	state: &mut State,
	pos: egui::Pos2,
) {
	for pressed in [true, false] {
		banner_frame(
			ctx,
			view,
			state,
			vec![
				egui::Event::PointerMoved(pos),
				egui::Event::PointerButton {
					pos,
					button: egui::PointerButton::Primary,
					pressed,
					modifiers: egui::Modifiers::NONE,
				},
			],
			false,
		);
	}
}

pub(super) fn check_channel_rename_heights() {
	let message = Message {
		poll: None,
		sticker_items: vec![],
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
		content: "<#4> ".repeat(12),
		author_nick: None,
		author_roles: vec![],
		mention_roles: vec![],
		mention_everyone: false,
		suppress_notifications: false,
		mentions: vec![],
		reactions: Some(vec![]),
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
		embeds_suppressed: false,
		attachments: vec![],
	};
	let message_key = layout_key(&message);
	let mut tail = message.clone();
	tail.id = Id(2);
	tail.content = "ordinary text ".repeat(350);
	let mut state = State {
		demo: true,
		selected: Some(Id(2)),
		revision: 1,
		..Default::default()
	};
	state.channels.push(model::Channel {
		id: Id(4),
		guild: Some(Id(5)),
		name: "a".into(),
		kind: 0,
		parent_id: None,
		position: 0,
		recipients: vec![],
		member_list_id: None,
		tags: None,
		message_count: None,
		icon: None,
		last_message: None,
	});
	// Channel names are shown only with view and read permission.
	state.user = Some(model::User {
		id: Id(9),
		name: "Synthetic viewer".into(),
		avatar: None,
		webhook: false,
		kind: Default::default(),
		discriminator: 0,
		primary_guild: None,
		cosmetics: None,
	});
	state.guilds.push(model::Guild {
		id: Id(5),
		name: "Synthetic".into(),
		icon: None,
		emojis: None,
		stickers: None,
		default_message_notifications: None,
	});
	state
		.permissions
		.replace(test_support::permission_snapshot(&state))
		.unwrap();
	state.timeline.insert(message, false, false).unwrap();
	state.timeline.insert(tail, false, false).unwrap();
	let mut view = TimelineView {
		channel: state.selected,
		anchor: Some((Id(1), 0.0)),
		..Default::default()
	};
	let mut images = crate::avatars::Avatars::default();
	let context = egui::Context::default();
	let render =
		|view: &mut TimelineView, state: &mut State, images: &mut crate::avatars::Avatars| {
			context
				.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(360.0, 300.0),
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
				)
				.drop_without_applying_deltas();
		};
	for _ in 0..3 {
		render(&mut view, &mut state, &mut images);
	}
	let short_height = view.heights[&Id(1)].1;
	state.apply(client_core::Envelope {
		generation: state.generation,
		event: client_core::Event::Message(test_support::message(1_000_000, Id(4))),
	});
	render(&mut view, &mut state, &mut images);
	assert_eq!(view.heights[&Id(1)].1, short_height);
	view.following = false;
	view.anchor = Some((Id(2), 400.0));
	view.revision = u64::MAX;
	for _ in 0..3 {
		render(&mut view, &mut state, &mut images);
	}
	assert_eq!(view.heights[&Id(1)].1, short_height);
	assert_eq!(view.anchor.unwrap().0, Id(2));
	state.apply(client_core::Envelope {
		generation: state.generation,
		event: client_core::Event::ChannelChanged(model::ChannelPatch {
			id: Id(4),
			name: model::Patch::Value("a-much-longer-channel-reference".into()),
			last_message: model::Patch::Absent,
			parent_id: model::Patch::Absent,
			position: model::Patch::Absent,
			kind: model::Patch::Absent,
			message_count: model::Patch::Absent,
			tags: model::Patch::Absent,
			icon: model::Patch::Absent,
		}),
	});
	assert_eq!(layout_key(state.timeline.get(Id(1)).unwrap()), message_key);
	render(&mut view, &mut state, &mut images);
	assert!(
		!view.heights.contains_key(&Id(1)),
		"An offscreen row must lose its old label-dependent height even though its message did not change"
	);
	view.following = false;
	view.anchor = Some((Id(1), 0.0));
	view.revision = u64::MAX;
	for _ in 0..3 {
		render(&mut view, &mut state, &mut images);
	}
	assert!(
		view.heights[&Id(1)].1 > short_height + 20.0,
		"The renamed references must be measured with their new wrapped labels"
	);
	assert!(images.take_requests().is_empty());
}

pub(super) fn label_ys(output: &egui::FullOutput) -> BTreeMap<String, f32> {
	fn walk(shape: &egui::Shape, out: &mut BTreeMap<String, f32>) {
		match shape {
			egui::Shape::Text(text) => {
				out.insert(text.galley.job.text.clone(), text.pos.y);
			}
			egui::Shape::Vec(shapes) => {
				for shape in shapes {
					walk(shape, out);
				}
			}
			_ => {}
		}
	}
	let mut labels = BTreeMap::new();
	for shape in &output.shapes {
		if shape.clip_rect.is_positive() {
			walk(&shape.shape, &mut labels);
		}
	}
	labels
}

pub(super) fn channel_messages(channel: u64, count: u64) -> State {
	let mut state = State {
		selected: Some(Id(channel)),
		revision: 1,
		demo: true,
		older_exhausted: true,
		freshness: model::Freshness::Fresh,
		channels: vec![model::Channel {
			id: Id(channel),
			guild: None,
			parent_id: None,
			position: 0,
			name: "general".into(),
			kind: 1,
			recipients: vec![],
			member_list_id: None,
			tags: None,
			message_count: None,
			icon: None,
			last_message: (count > 0).then_some(Id(count)),
		}],
		..Default::default()
	};
	for id in 1..=count {
		let mut message = text_message(id);
		message.channel = Id(channel);
		message.content = format!("Row {id}");
		state.timeline.insert(message, false, false).unwrap();
	}
	state
}

pub(super) fn paint_timeline(
	ctx: &egui::Context,
	view: &mut TimelineView,
	state: &mut State,
	frame: u32,
) -> BTreeMap<String, f32> {
	let mut avatars = crate::avatars::Avatars::default();
	let output = ctx.run_ui(
		egui::RawInput {
			screen_rect: Some(egui::Rect::from_min_size(
				egui::Pos2::ZERO,
				egui::vec2(900.0, 600.0),
			)),
			time: Some(f64::from(frame) / 60.0),
			focused: true,
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
	);
	let labels = label_ys(&output);
	output.drop_without_applying_deltas();
	labels
}

pub(super) fn newest_y(labels: &BTreeMap<String, f32>, count: u64) -> Option<f32> {
	labels.get(&format!("Row {count}")).copied()
}
