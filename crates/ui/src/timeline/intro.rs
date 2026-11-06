use super::*;

/// Row id of the direct-message introduction; no message uses snowflake 0.
pub(super) const INTRO: Id = Id(0);
pub(super) const INTRO_ESTIMATE: f32 = 236.0;
/// What leads a conversation once its whole history is loaded, as in the official client.
#[derive(Clone)]
pub(super) enum Intro {
	/// The other person in a one-to-one conversation.
	Direct(model::User),
	/// A group direct message, looked up when painted.
	Group(Id),
	/// A server channel, thread or forum post with messages.
	Channel { name: String, kind: u8, post: bool },
}
/// The loaded window reaches the first message. Finished older pages keep
/// `history_before` set, so it does not disqualify the window.
/// The loaded rows begin at the conversation's first message. Newer messages need not be
/// loaded: scrolling up through a long history, or opening a search result, still reaches it.
pub(super) fn history_complete(state: &State) -> bool {
	state.freshness == model::Freshness::Fresh
		&& !state.history_pending
		&& state.older_exhausted
		&& !state.timeline.older_evicted()
}
/// A cached window shown while its first network page loads, already known to reach the start.
pub(super) fn cached_start(state: &State) -> bool {
	state.freshness == model::Freshness::Loading
		&& state.history_pending
		&& !state.history_targeted
		&& state.history_after.is_none()
		&& state.older_exhausted
		&& state.timeline.row_count() != 0
}
pub(super) fn intro(state: &State, empty: bool) -> Option<Intro> {
	let channel = state.selected.and_then(|id| state.channel(id))?;
	if !history_complete(state) && !cached_start(state) {
		return None;
	}
	let named = |post| Intro::Channel {
		name: channel.name.clone(),
		kind: channel.kind,
		post,
	};
	match channel.kind {
		1 => channel.recipients.first().cloned().map(Intro::Direct),
		3 if channel.guild.is_none() => Some(Intro::Group(channel.id)),
		// An empty server channel keeps its full-height welcome instead.
		0 | 5 if channel.guild.is_some() && !empty => Some(named(false)),
		10..=12 if !empty => {
			let post = channel
				.parent_id
				.and_then(|parent| state.channel(parent))
				.is_some_and(|parent| matches!(parent.kind, 15 | 16));
			Some(named(post))
		}
		_ => None,
	}
}
/// Height key for the introduction: its layout depends on what it names.
pub(super) fn intro_key(intro: &Intro, state: &State) -> u64 {
	let mut hasher = DefaultHasher::new();
	match intro {
		Intro::Direct(user) => {
			user.id.hash(&mut hasher);
			user.name.hash(&mut hasher);
			let heading = state.user_display_name(user);
			heading.hash(&mut hasher);
			intro_username(state, user, heading).hash(&mut hasher);
			state
				.known_profile(user.id)
				.map(|profile| profile.mutual_guilds.len())
				.hash(&mut hasher);
		}
		Intro::Channel { name, kind, post } => {
			(name, kind, post).hash(&mut hasher);
		}
		Intro::Group(id) => {
			id.hash(&mut hasher);
			if let Some(channel) = state.channel(*id) {
				state.conversation_name(channel).hash(&mut hasher);
			}
		}
	}
	hasher.finish()
}
/// "Welcome to #name!" for a server channel, or a thread's or forum post's title.
pub(super) fn channel_intro(ui: &mut egui::Ui, name: &str, kind: u8, post: bool, width: f32) {
	let colors = crate::design::palette(ui);
	egui::Frame::NONE
		.inner_margin(egui::Margin {
			left: 16,
			right: 16,
			top: 16,
			bottom: 4,
		})
		.show(ui, |ui| {
			ui.set_min_width((width - 32.0).max(1.0));
			ui.spacing_mut().item_spacing.y = 8.0;
			let (badge, _) = ui.allocate_exact_size(egui::Vec2::splat(68.0), egui::Sense::hover());
			let icon = match kind {
				5 => crate::icons::Icon::Megaphone,
				10..=12 if post => crate::icons::Icon::Forum,
				10..=12 => crate::icons::Icon::Thread,
				_ => crate::icons::Icon::Hash,
			};
			if !post {
				ui.painter()
					.circle_filled(badge.center(), 34.0, colors.raised);
			}
			crate::icons::paint(
				ui.painter(),
				icon,
				badge.shrink(if post { 8.0 } else { 16.0 }),
				colors.text_strong,
			);
			let thread = matches!(kind, 10..=12);
			let title = if thread {
				name.to_owned()
			} else {
				crate::i18n::translate_args("timeline-channel-intro-title", &[("channel", name)])
			};
			ui.add(
				egui::Label::new(
					crate::design::semibold(ui, title, if thread { 30.0 } else { 32.0 })
						.color(colors.text_strong),
				)
				.wrap()
				.selectable(false),
			);
			if !thread {
				ui.label(
					RichText::new(crate::i18n::translate_args(
						"timeline-channel-intro-start",
						&[("channel", name)],
					))
					.size(15.0)
					.color(colors.muted),
				);
			}
		});
}
pub(super) fn intro_username(state: &State, user: &model::User, heading: &str) -> Option<String> {
	crate::profiles::known_username(state, user.id).filter(|username| username != heading)
}
/// One intro sentence with the conversation's name in bold.
pub(super) fn intro_sentence(ui: &mut egui::Ui, key: &str, name: &str) {
	let colors = crate::design::palette(ui);
	let text = crate::i18n::translate_args(key, &[("name", name)]);
	let mut job = egui::text::LayoutJob::default();
	let plain = egui::TextFormat::simple(egui::FontId::proportional(15.0), colors.text);
	let bold = egui::TextFormat::simple(
		egui::FontId::new(15.0, crate::design::semibold_family(ui.ctx())),
		colors.text_strong,
	);
	match text.rfind(name).filter(|_| !name.is_empty()) {
		Some(at) => {
			job.append(&text[..at], 0.0, plain.clone());
			job.append(name, 0.0, bold);
			job.append(&text[at + name.len()..], 0.0, plain);
		}
		None => job.append(&text, 0.0, plain),
	}
	ui.add(egui::Label::new(job).wrap().selectable(false));
}
/// "Welcome to the beginning of the … group" header, as in the official client.
pub(super) fn group_intro(
	ui: &mut egui::Ui,
	channel: &model::Channel,
	state: &State,
	avatars: &mut crate::avatars::Avatars,
	width: f32,
) {
	let colors = crate::design::palette(ui);
	egui::Frame::NONE
		.inner_margin(egui::Margin {
			left: 16,
			right: 16,
			top: 16,
			bottom: 4,
		})
		.show(ui, |ui| {
			ui.set_min_width((width - 32.0).max(1.0));
			ui.spacing_mut().item_spacing.y = 8.0;
			avatars.show_group(ui, channel, 80.0, state.demo);
			let name = state.conversation_name(channel);
			ui.add(
				egui::Label::new(crate::design::semibold(ui, name, 30.0).color(colors.text_strong))
					.wrap()
					.selectable(false),
			);
			intro_sentence(ui, "timeline-group-intro", name);
		});
}
/// "This is the beginning of your direct message history" header, as in the official client.
pub(super) fn dm_intro(
	ui: &mut egui::Ui,
	user: &model::User,
	state: &State,
	avatars: &mut crate::avatars::Avatars,
	profile: &mut crate::profiles::ProfileSession,
	width: f32,
) -> Option<crate::user_menu::Action> {
	use crate::user_menu::Action;
	let colors = crate::design::palette(ui);
	let mut action = None;
	egui::Frame::NONE
		.inner_margin(egui::Margin {
			left: 16,
			right: 16,
			top: 16,
			bottom: 4,
		})
		.show(ui, |ui| {
			ui.set_min_width((width - 32.0).max(1.0));
			ui.spacing_mut().item_spacing.y = 8.0;
			let avatar = avatars.show(ui, user, 80.0, state.demo);
			profile.person_click(ui, &avatar, None, user);
			let name = state.user_display_name(user).to_owned();
			let heading = ui.add(
				egui::Label::new(
					crate::design::semibold(ui, name.as_str(), 30.0).color(colors.text_strong),
				)
				.sense(egui::Sense::click())
				.selectable(false),
			);
			profile.person_click(ui, &heading, None, user);
			// The username is shown only when known and when it adds to the heading.
			if let Some(username) = intro_username(state, user, &name) {
				ui.label(RichText::new(username).size(20.0).color(colors.text));
			}
			intro_sentence(ui, "timeline-dm-intro", &name);
			ui.horizontal_wrapped(|ui| {
				ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
				if let Some(known) = state.known_profile(user.id)
					&& !known.mutual_guilds.is_empty()
				{
					ui.horizontal(|ui| {
						ui.spacing_mut().item_spacing.x = -4.0;
						for mutual in known.mutual_guilds.iter().take(3) {
							if let Some(guild) = state.guilds.iter().find(|g| g.id == mutual.id) {
								let (rect, _) = ui.allocate_exact_size(
									egui::Vec2::splat(24.0),
									egui::Sense::hover(),
								);
								ui.painter().circle_filled(rect.center(), 13.0, colors.chat);
								avatars.paint_guild(ui, guild, rect, state.demo, 12);
							}
						}
					});
					let count = known.mutual_guilds.len();
					let label = crate::i18n::translate(if count == 1 {
						"profiles-show-mutual-server"
					} else {
						"profiles-show-mutual-servers"
					});
					ui.label(
						RichText::new(format!("{count} {label}"))
							.size(14.0)
							.color(colors.muted),
					);
					ui.label(RichText::new("•").color(colors.muted));
				}
				let enabled = crate::profiles::actions_enabled(state);
				let button = |ui: &mut egui::Ui,
				              label: &str,
				              fill: egui::Color32,
				              text: egui::Color32,
				              on: bool| {
					ui.add_enabled(
						on,
						egui::Button::new(RichText::new(label).size(14.0).color(text))
							.fill(fill)
							.corner_radius(8)
							.min_size(egui::vec2(0.0, 32.0)),
					)
				};
				let blocked = state.user_blocked(user.id) == Some(true);
				if crate::profiles::friend_target(state, user) && !blocked {
					let friend = state.friends().any(|friend| friend.id == user.id);
					let request = state
						.pending_friends()
						.find(|(person, _, _)| person.id == user.id)
						.map(|(_, _, incoming)| *incoming);
					let known = state.friends_known() && state.friend_requests_known();
					if !friend {
						let (label, wanted, primary) = match request {
							Some(true) => (
								"profiles-friend-action-accept",
								Some(Action::AcceptFriend(user.id)),
								true,
							),
							Some(false) => ("profiles-friend-action-sent", None, false),
							None => ("friends-add", Some(Action::AddFriend(user.clone())), true),
						};
						let (fill, text) = if primary {
							(colors.accent, colors.accent_text)
						} else {
							(colors.raised, colors.text)
						};
						if button(
							ui,
							&crate::i18n::translate(label),
							fill,
							text,
							enabled && known && wanted.is_some(),
						)
						.clicked()
						{
							action = wanted;
						}
					}
				}
				if crate::profiles::friend_target(state, user)
					&& state.user_blocked(user.id).is_some()
				{
					let label = crate::i18n::translate(if blocked {
						"user-menu-contents-unblock"
					} else {
						"user-menu-contents-block"
					});
					if button(ui, &label, colors.raised, colors.text_strong, enabled).clicked() {
						action = Some(Action::Block {
							user: user.id,
							blocked: !blocked,
						});
					}
				}
			});
		});
	action
}
