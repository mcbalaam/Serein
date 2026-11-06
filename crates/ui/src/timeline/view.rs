use super::*;

impl TimelineView {
	pub(crate) fn show_fullscreen_video(&mut self, ctx: &egui::Context, state: &State) -> bool {
		if self.video.is_fullscreen() {
			let current = self
				.video
				.active
				.as_ref()
				.and_then(|(channel, id, attachment)| {
					state
						.timeline
						.get(*id)
						.or_else(|| {
							state
								.interactions
								.ephemeral
								.iter()
								.find(|message| message.id == *id)
						})
						.filter(|message| {
							state.selected == Some(*channel)
								&& message.channel == *channel
								&& message.attachments.contains(attachment)
								&& (self.component_viewing
									== Some((
										message.id,
										egui::Id::unique((
											&message.components,
											&message.attachments,
										))
										.value(),
									)) || !crate::embeds::has_media_spoilers(message)
									|| self.revealed.get(id).is_some_and(|reveal| {
										reveal.media && reveal.matches(message)
									}))
						})
						.map(|message| (message, attachment.clone()))
				});
			if let Some((message, attachment)) = current {
				self.video.show_fullscreen(
					ctx,
					message,
					&attachment,
					&mut self.download,
					&mut self.opening,
					state.demo,
				);
				return true;
			} else {
				self.video.stop();
			}
		}
		false
	}

	/// Fixture-only: open the media viewer on one attachment.
	#[cfg(any(test, feature = "demo"))]
	pub(crate) fn preview_image_viewer(&mut self, message: Id, attachment: Id) {
		self.pending_viewer = Some((message, attachment));
	}
	pub(crate) fn viewing_latest(&self, channel: Id) -> bool {
		self.channel == Some(channel) && self.following && self.at_current_latest
	}
	/// An explicit "Mark unread" starts a new section at the chosen message.
	pub(crate) fn reset_unread_divider(&mut self) {
		self.unread_boundary = None;
		self.unread_session = false;
		self.unread_dismissed = None;
		self.revision = u64::MAX;
	}
	/// Newest message the reader has already read during this visit, when it lies inside the
	/// divider's section: the banner counts only what arrived after it.
	pub(crate) fn read_through(&self) -> Option<Id> {
		self.unread_dismissed.flatten().filter(|read| {
			self.unread_boundary
				.is_some_and(|boundary| boundary <= *read)
		})
	}
	/// "12 new messages since 14:05" for the divider's section; "50+" when older unread
	/// messages are not loaded yet. Messages read during the visit are not counted again.
	pub(crate) fn unread_summary(&self, state: &State) -> Option<String> {
		let boundary = self.unread_boundary?;
		let read_through = self.read_through();
		let start = match read_through {
			Some(read) => state.timeline.iter().find(|m| m.id > read)?.id,
			None => boundary,
		};
		let count = state.timeline.iter().filter(|m| m.id >= start).count();
		let first_loaded = state.timeline.iter().next().map(|m| m.id) == Some(boundary);
		let marker_loaded = state
			.selected
			.and_then(|channel| state.read_marker(channel))
			.flatten()
			.is_some_and(|read| state.timeline.get(read).is_some());
		let more =
			read_through.is_none() && first_loaded && !marker_loaded && !state.older_exhausted;
		let at = timestamp(start);
		let clock = crate::local_time::clock(at);
		let time = if at.date() == crate::local_time::now().date() {
			clock
		} else {
			format!("{} {}, {clock}", at.month(), at.day())
		};
		Some(crate::i18n::translate_count(
			if more {
				"timeline-unread-banner-new-since-more"
			} else {
				"timeline-unread-banner-new-since"
			},
			count,
			&[("time", &time)],
		))
	}
	/// Leaving the latest page is deliberate reading; nothing is acknowledged automatically.
	pub(crate) fn browse_away(&mut self) {
		self.target_browsing = true;
		self.following = false;
		self.jump = false;
		self.reveal_scroll = None;
		self.present_scroll = None;
		self.pending_reveal = None;
		self.auto_read_attempt = None;
		self.mark_read = None;
		self.mark_unread = None;
		self.seen_latest = None;
	}
	pub(crate) fn follow_latest(&mut self, state: &State) {
		self.latest |= state.history_targeted
			|| state.history_before.is_some()
			|| state.history_after.is_some();
		self.target_browsing = false;
		self.hold_read_ack = false;
		self.auto_read_attempt = None;
		self.following = true;
		self.jump = true;
		self.anchor = None;
		self.reveal_scroll = None;
		self.present_scroll = None;
		self.pending_reveal = None;
	}
	pub(crate) fn request_reply_target(&mut self, id: Id) {
		self.present_scroll = None;
		self.reply_target = Some(id);
		self.pending_reveal = Some(if self.following && self.at_current_latest {
			TargetReveal::StayIfVisible
		} else {
			TargetReveal::Center
		});
	}
	/// Render the message timeline using a default scroll session (test helper).
	#[cfg(test)]
	pub fn show(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		editing: &mut Option<(Id, Id, String)>,
		deleting: &mut Option<(Id, Id)>,
		(avatars, profile): (
			&mut crate::avatars::Avatars,
			&mut crate::profiles::ProfileSession,
		),
		upload: Option<&crate::pending::Upload>,
	) {
		let mut scroll = crate::scroll::Session::default();
		self.show_with_scroll(
			ui,
			state,
			editing,
			deleting,
			(avatars, profile),
			upload,
			&mut scroll,
		);
	}

	/// Render the message timeline within the active scroll session.
	#[allow(clippy::too_many_arguments)]
	pub fn show_with_scroll(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		editing: &mut Option<(Id, Id, String)>,
		deleting: &mut Option<(Id, Id)>,
		(avatars, profile): (
			&mut crate::avatars::Avatars,
			&mut crate::profiles::ProfileSession,
		),
		upload: Option<&crate::pending::Upload>,
		session: &mut crate::scroll::Session,
	) {
		let width = ui.available_width();
		let channel_changed = self.channel != state.selected;
		if channel_changed {
			if let Some(previous) = self.channel {
				let cursor = if self.following {
					client_core::ReadingCursor {
						message: None,
						inset: 0.0,
					}
				} else {
					client_core::ReadingCursor {
						message: self.anchor.map(|(id, _)| id),
						inset: self.anchor.map(|(_, inset)| inset).unwrap_or(0.0),
					}
				};
				state.remember_reading(previous, cursor);
			}
			let restored = state.selected.and_then(|id| state.reading(id));
			let following = restored.is_none_or(|cursor| cursor.message.is_none());
			*self = Self {
				extension_actions: self.extension_actions.clone(),
				quick_reactions: self.quick_reactions,
				double_click_reaction: self.double_click_reaction,
				compact_messages: self.compact_messages,
				hide_media_links: self.hide_media_links,
				instant_scrolling: self.instant_scrolling,
				suppressed_deleted_highlight: std::mem::take(
					&mut self.suppressed_deleted_highlight,
				),
				channel: state.selected,
				following,
				anchor: restored.and_then(|cursor| cursor.message.map(|id| (id, cursor.inset))),
				download: std::mem::take(&mut self.download),
				// Posters survive the switch so idle stages repaint from cache.
				// Playback still halts: the unseen player hits the idle stop.
				video: {
					let mut video = std::mem::take(&mut self.video);
					video.stop();
					video.command = None;
					video
				},
				opening: self.opening.take(),
				browser_opening: self.browser_opening.take(),
				pending_viewer: self.pending_viewer.take(),
				jump: following,
				leave_read: self.channel.zip(self.seen_latest),
				..Self::default()
			};
		}
		if !self.initial_read_checked
			&& state.freshness == model::Freshness::Fresh
			&& !state.history_pending
			&& let Some(channel) = state.selected
			&& let Some(unread) = state.unread(channel)
		{
			self.initial_read_checked = true;
			if unread {
				self.mark_read = None;
				// Opening an unread conversation at its latest messages reads it, as in the
				// official client: the banner clears while the divider keeps its place for
				// this visit. Only an unread section that starts above the loaded page keeps
				// its banner, since Jump to unread is then the way to its first message.
				let arrived_on_live_edge = self.following
					&& state
						.channel(channel)
						.is_some_and(|channel| channel.supports_text())
					&& state.timeline.iter().next().is_some();
				let section_loaded = state.older_exhausted
					|| matches!(state.read_marker(channel), Some(Some(read))
						if state.timeline.iter().next().is_some_and(|first| first.id <= read));
				if arrived_on_live_edge && !section_loaded {
					self.hold_read_ack = true;
				}
			}
		}
		let can_load_newer = state.can_load_newer();
		if state
			.selected
			.is_some_and(|channel| state.missed(channel) == Some(false))
		{
			self.hold_read_ack = false;
		}
		if self.unread_jump || self.load_newer {
			self.browse_away();
		}
		// Incoming messages being watched at the live edge are not a new unread section.
		// Keep service read state unchanged while its acknowledgement is in flight.
		let watching_latest = self.following
			&& self.at_current_latest
			&& state.live_edge_latest().is_some()
			&& ui.input(|input| input.focused);
		let first_unread = state
			.selected
			.and_then(|channel| state.read_marker(channel))
			.and_then(|read| {
				state
					.timeline
					.iter()
					.find(|m| read.is_none_or(|id| m.id > id))
					.map(|m| m.id)
			});
		let kept = self
			.unread_boundary
			.filter(|id| state.timeline.get(*id).is_some());
		// The divider is fixed for the visit. Acknowledging moves the read marker to the latest
		// message, so the next arrival would otherwise look like a new first unread message;
		// only older unread history loading above it may move the divider up. Once the reader
		// has read past it, messages that then arrive unseen start a new section.
		let unseen_after_read = first_unread.filter(|first| {
			!watching_latest
				&& self
					.read_through()
					.is_some_and(|read| kept.is_some_and(|kept| kept <= read) && *first > read)
		});
		let boundary = match kept {
			Some(_) if unseen_after_read.is_some() => unseen_after_read,
			Some(kept) => Some(first_unread.map_or(kept, |first| first.min(kept))),
			None => first_unread.filter(|_| !watching_latest),
		};
		if self.unread_boundary != boundary {
			self.unread_boundary = boundary;
			self.revision = u64::MAX;
		}
		let text_size = egui::TextStyle::Body.resolve(ui.style()).size;
		let font_revision = crate::fonts::revision(ui.ctx());
		let scale = ui.ctx().pixels_per_point();
		let width_changed = (self.width - width).abs() > 1.0;
		let content_dimensions_changed = self.text_size != text_size
			|| self.font_revision != font_revision
			|| self.scale != scale
			|| self.hide_media_links != self.applied_hide_media_links
			|| self.compact_messages != self.applied_compact_messages;
		let dimensions_changed = width_changed || content_dimensions_changed;
		self.applied_hide_media_links = self.hide_media_links;
		self.applied_compact_messages = self.compact_messages;
		// A thread's starter joins the rows only once the whole thread history is loaded.
		let starter = state.thread_starter().filter(|_| history_complete(state));
		let starter_id = starter.map(|m| m.id);
		let intro_empty = state.timeline.display_iter().next().is_none()
			&& !state
				.pending
				.iter()
				.any(|p| Some(p.channel) == state.selected);
		let intro = intro(state, intro_empty);
		let intro_height_key = intro.as_ref().map(|intro| intro_key(intro, state));
		let mut layout_changed = false;
		if self.revision != state.revision {
			// Member/presence and other unrelated updates must not restore the scroll
			// anchor or reintroduce estimates for already settled message geometry.
			let mut fingerprint = DefaultHasher::new();
			let mut previous = None;
			for message in starter.into_iter().chain(display_rows(state)) {
				let deleted = state.timeline.is_deleted(message.id);
				message.id.hash(&mut fingerprint);
				row_key(message, previous, self.unread_boundary, state).hash(&mut fingerprint);
				deleted.hash(&mut fingerprint);
				previous = Some(message);
			}
			let fingerprint = fingerprint.finish();
			layout_changed = self.layout_fingerprint != fingerprint;
			self.layout_fingerprint = fingerprint;
		}
		let changed = self.revision == u64::MAX
			|| layout_changed
			|| dimensions_changed
			|| self.starter_row != starter_id
			|| self.intro_row != intro.is_some()
			|| intro_height_key
				.is_some_and(|key| self.heights.get(&INTRO).is_some_and(|(old, _)| *old != key));
		self.starter_row = starter_id;
		self.intro_row = intro.is_some();
		if changed || self.revision != state.revision || self.width != width {
			self.measured_rows.clear();
		}
		let mut offset = None;
		let mut lead_rows = None;
		if changed {
			if content_dimensions_changed {
				self.heights.clear();
				self.pending_heights.clear();
			}
			// Keep measured heights as estimates during resize. Visible rows are
			// remeasured below; resetting everything makes the scroll extent jump.
			self.width = width;
			self.text_size = text_size;
			self.font_revision = font_revision;
			self.scale = scale;
			let row_ids: Vec<_> = intro
				.as_ref()
				.map(|_| INTRO)
				.into_iter()
				.chain(starter_id)
				.chain(display_rows(state).map(|message| message.id))
				.collect();
			self.heights
				.retain(|id, _| row_ids.binary_search(id).is_ok());
			self.suppressed_deleted_highlight
				.retain(|id| row_ids.binary_search(id).is_ok());
			self.formatted
				.retain(|id| display_message(state, id).is_some());
			self.toolbar = self
				.toolbar
				.filter(|(id, _)| display_message(state, *id).is_some());
			self.revealed.retain(|id, content| {
				display_message(state, *id).is_some_and(|m| content.matches(m))
			});
			let mut previous = None;
			let mut lead_basis = 0.0;
			let mut stale_heights = Vec::new();
			self.rows = starter
				.into_iter()
				.chain(display_rows(state))
				.map(|m| {
					let prior = previous;
					let deleted = state.timeline.is_deleted(m.id);
					let key = row_height_key(m, prior, self.unread_boundary, state, deleted);
					previous = Some(m);
					let lines = m
						.content
						.lines()
						.map(|line| {
							(line.chars().count() as f32 / ((width - 80.0) / 8.0).max(1.0))
								.ceil()
								.max(1.0)
						})
						.sum::<f32>();
					let mut estimate = (if m.embeds_suppressed {
						0.0
					} else {
						crate::embeds::estimated_height(&m.embeds)
							+ crate::invites::estimated_height(m)
					}) + crate::attachments::estimated_height(
						&m.attachments,
						(width - 88.0).max(1.0),
					) + 58.0 + 18.0 * lines.min(128.0);
					if self.compact_messages || grouped(prior, m, self.unread_boundary) {
						estimate = (estimate - GROUPED_ROW_SAVINGS).max(24.0);
					}
					estimate += reserved_chrome(ui, m, width);
					let height = match self.heights.get(&m.id) {
						Some((old_key, height)) if *old_key == key => *height,
						Some(_) => {
							stale_heights.push(m.id);
							estimate
						}
						None => estimate,
					};
					lead_basis += if height * 8.0 < estimate {
						estimate
					} else {
						height
					};
					(m.id, height)
				})
				.collect();
			for id in stale_heights {
				self.heights.remove(&id);
			}
			if let Some(key) = intro_height_key {
				let height = match self.heights.get(&INTRO) {
					Some((old, height)) if *old == key => *height,
					_ => INTRO_ESTIMATE,
				};
				self.rows.insert(0, (INTRO, height));
				lead_basis += height;
			}
			lead_rows = Some(lead_basis);
			if !self.following
				&& let Some((id, inset)) = self.anchor
			{
				offset = Some(anchor_offset(&self.rows, id, inset));
			}
		}
		self.revision = state.revision;
		let history_available = state
			.selected
			.is_some_and(|channel| state.can_read_history(channel));
		let empty = state.timeline.display_iter().next().is_none()
			&& !state
				.pending
				.iter()
				.any(|p| Some(p.channel) == state.selected);
		// An empty historical page or a pending refresh is not proof of a new channel.
		let welcome = empty
			&& history_available
			&& state.freshness == model::Freshness::Fresh
			&& !state.history_pending
			&& !state.history_targeted
			&& state.history_before.is_none()
			&& state.history_after.is_none()
			&& state.older_exhausted
			&& state
				.selected
				.and_then(|id| state.channel(id))
				.is_some_and(|channel| channel.guild.is_some() && channel.supports_text());
		if !history_available {
			ui.weak(crate::i18n::translate(
				"timeline-show-with-scroll-message-history-is-unavailable-with-current-permission-information",
			));
		}
		if history_available && state.freshness == model::Freshness::Loading && empty {
			let area = ui.available_rect_before_wrap().intersect(ui.clip_rect());
			loading_messages(ui);
			session.bind(ui, ui.scope_id().with(("timeline", state.selected)), area);
			let latest = state
				.selected
				.and_then(|channel| state.channel(channel))
				.and_then(|channel| channel.last_message);
			if state.show_missed_banner() && self.unread_dismissed != Some(latest) {
				let (jump_unread, mark_read) =
					unread_banner(ui, banner_rect(area), state.can_jump_unread(), None);
				if jump_unread {
					self.unread_jump = true;
					self.browse_away();
				}
				if jump_unread || mark_read {
					self.unread_dismissed = Some(latest);
				}
				if mark_read {
					self.hold_read_ack = false;
					self.mark_channel_read = state.selected;
				}
			}
			return;
		} else if empty
			&& history_available
			&& !welcome
			&& !matches!(intro, Some(Intro::Direct(_) | Intro::Group(_)))
		{
			ui.label(crate::i18n::translate_if_key(match state.freshness {
				model::Freshness::Loading => "timeline-show-with-scroll-loading-messages",
				model::Freshness::Unavailable => {
					"timeline-show-with-scroll-you-cannot-view-this-conversation"
				}
				model::Freshness::Stale => {
					"timeline-show-with-scroll-history-is-not-available-yet-use-reload-to-try-again"
				}
				model::Freshness::Fresh => {
					"timeline-show-with-scroll-no-messages-yet-start-the-conversation-below"
				}
			}));
		}
		let area = ui.available_rect_before_wrap().intersect(ui.clip_rect());
		let autoscroll_delta =
			session.bind(ui, ui.scope_id().with(("timeline", state.selected)), area);
		if autoscroll_delta > 0.0 {
			self.following = false;
		}
		let total: f32 = self.rows.iter().map(|(_, height)| height).sum();
		// The typing indicator floats in the reserved strip above the composer; the gap keeps
		// it from covering the last message, and stays there when nobody is typing.
		let end_padding = 8.0 + crate::typing::OVERLAY_HEIGHT;
		self.pending_heights.retain(|nonce, _| {
			state
				.pending
				.iter()
				.any(|p| &p.nonce == nonce && Some(p.channel) == state.selected)
		});
		let pending_rows: Vec<_> = state
			.pending
			.iter()
			.filter(|p| Some(p.channel) == state.selected)
			.map(|p| {
				let height = self
					.pending_heights
					.get(&p.nonce)
					.copied()
					.unwrap_or_else(|| {
						80.0 + p
							.content
							.lines()
							.map(|line| {
								(line.chars().count() as f32 / ((width - 88.0) / 8.0).max(1.0))
									.ceil()
									.max(1.0) * 20.0
							})
							.sum::<f32>() + if p.attachments.is_empty() { 0.0 } else { 320.0 }
					});
				self.pending_heights
					.entry(p.nonce.clone())
					.or_insert(height);
				(p, height)
			})
			.collect();
		let pending_height = pending_rows.iter().map(|(_, height)| *height).sum::<f32>();
		let packed = total + pending_height + end_padding;
		let lead_packed = lead_rows.unwrap_or(total) + pending_height + end_padding;
		// Short content rests on the composer only at the newest messages. A window opened at a
		// target or paging towards newer messages starts at the top, like Discord, so the
		// history's start is not pushed down and loading newer pages does not shift it.
		let live_edge =
			!state.history_targeted && state.history_after.is_none() && !state.newer_may_have_more;
		if state.freshness == model::Freshness::Fresh && !state.history_pending {
			if let Some(target) = state.search_target.take() {
				let saved = state
					.restore_scroll
					.then(|| {
						state
							.selected
							.and_then(|channel| state.reading(channel))
							.filter(|cursor| cursor.message == Some(target))
					})
					.flatten();
				state.restore_scroll = false;
				let reveal = self.pending_reveal.take().unwrap_or(TargetReveal::Center);
				if state.timeline.get(target).is_some() {
					if saved.is_none() {
						self.highlighted = Some((target, ui.input(|input| input.time) + 2.0));
					}
					let viewport_h = area.height();
					let current_offset = self
						.scroll_offset
						.clamp(0.0, (packed - viewport_h).max(0.0));
					let (first, end, _) =
						visible_range(&self.rows, current_offset, current_offset + viewport_h);
					let visible = self.rows[first..end].iter().any(|(id, _)| *id == target);
					let stay = saved.is_none()
						&& match reveal {
							TargetReveal::StayIfVisible => visible,
							TargetReveal::Center => false,
						};
					if !stay {
						self.target_browsing = true;
						self.mark_read = None;
						self.following = false;
						self.jump = false;
						let to = if let Some(cursor) = saved {
							anchor_offset(&self.rows, target, cursor.inset)
								.clamp(0.0, (packed - viewport_h).max(0.0))
						} else {
							centered_offset(&self.rows, target, viewport_h, packed)
						};
						if saved.is_some()
							|| self.instant_scrolling
							|| (to - current_offset).abs() < 1.0
						{
							offset = Some(to);
							self.reveal_scroll = None;
						} else {
							self.reveal_scroll = Some(RevealScroll {
								target,
								from: current_offset,
								elapsed: 0.0,
							});
						}
					}
				} else {
					self.target_browsing = true;
					self.mark_read = None;
					state.status =
						"Message was not returned; it may have been removed or become unavailable";
				}
			} else {
				state.restore_scroll = false;
			}
		}
		if let Some((_, until)) = self.highlighted {
			let remaining = until - ui.input(|input| input.time);
			if remaining > 0.0 {
				ui.ctx()
					.request_repaint_after(std::time::Duration::from_secs_f64(remaining));
			} else {
				self.highlighted = None;
			}
		}
		let mut scroll = egui::ScrollArea::vertical()
			.id_salt(("timeline", state.selected))
			.auto_shrink([false, false])
			.animated(!self.instant_scrolling)
			.stick_to_bottom(self.following);
		let live_edge_offset =
			(total + end_padding + pending_rows.iter().map(|(_, height)| height).sum::<f32>()
				- ui.available_height())
			.max(0.0);
		let mut jumped_to = None;
		if std::mem::take(&mut self.jump) && self.following {
			self.reveal_scroll = None;
			self.present_scroll = None;
			offset = Some(live_edge_offset);
			jumped_to = Some(live_edge_offset);
		}
		if self.following {
			// egui sticks after painting; use this frame's height while the call panel resizes.
			offset = Some(live_edge_offset);
		}
		let wheel = ui.input(|input| input.smooth_scroll_delta());
		let user_scroll = wheel.y + autoscroll_delta;
		if user_scroll != 0.0 && self.reveal_scroll.take().is_some() {
			offset = None;
		}
		if user_scroll != 0.0 {
			self.present_scroll = None;
		}
		// Jump to present glides back to the live edge instead of teleporting there.
		if let Some((from, elapsed)) = &mut self.present_scroll {
			*elapsed += ui.input(|input| input.stable_dt).clamp(1.0 / 240.0, 0.05);
			let t = *elapsed / PRESENT_SCROLL_SECS;
			if t >= 1.0 {
				offset = Some(live_edge_offset);
				self.present_scroll = None;
				self.follow_latest(state);
				self.jump = false;
			} else {
				offset = Some(*from + (live_edge_offset - *from) * ease_out_cubic(t));
			}
			ui.ctx().request_repaint();
		}
		if let Some(motion) = &mut self.reveal_scroll {
			motion.elapsed += ui.input(|input| input.stable_dt).clamp(1.0 / 240.0, 0.05);
			let t = motion.elapsed / REVEAL_SCROLL_SECS;
			let to = centered_offset(&self.rows, motion.target, area.height(), packed);
			if t >= 1.0 {
				offset = Some(to);
				self.reveal_scroll = None;
			} else {
				offset = Some(motion.from + (to - motion.from) * ease_out_cubic(t));
				ui.ctx().request_repaint();
			}
		}
		if autoscroll_delta != 0.0 {
			// Set the viewport before virtualization. A global scroll delta can be consumed
			// by nested embed scroll areas and moves past the rows laid out this frame.
			let max_offset = live_edge_offset;
			let next =
				(offset.unwrap_or(self.scroll_offset) - autoscroll_delta).clamp(0.0, max_offset);
			offset = Some(next);
			if next != self.scroll_offset {
				ui.ctx().request_repaint();
			}
		}
		if let Some(offset) = offset {
			scroll = scroll.vertical_scroll_offset(offset);
		}
		self.visible_authors.clear();
		#[cfg(test)]
		{
			self.leading_rendered = 0;
		}
		let mut measurements = Vec::new();
		let mut intro_action = None;
		let mut selected_reply = None;
		// ScrollArea consumes wheel input while applying it; retain the viewing gesture.
		let scroll_delta = wheel.y + autoscroll_delta;
		let allow_hover = !session.holding()
			&& !ui.input(|input| input.is_scrolling())
			&& ui.ctx().dragged_id().is_none();
		let output = scroll.show_viewport(ui, |ui, viewport| {
			ui.spacing_mut().item_spacing.y = 0.0;
			if welcome && let Some(channel) = state.selected.and_then(|id| state.channel(id)) {
				channel_welcome(ui, channel, viewport.height() - end_padding);
			}
			let lead = if welcome || !live_edge {
				0.0
			} else {
				(viewport.height() - lead_packed).max(0.0)
			};
			ui.add_space(lead);
			let overscan = 100.0;
			let (first, _, top) = visible_range(
				&self.rows,
				(viewport.min.y - overscan - lead).max(0.0),
				(viewport.max.y + 100.0 - lead).max(0.0),
			);
			let (anchor, _, anchor_top) = visible_range(
				&self.rows,
				(viewport.min.y - lead).max(0.0),
				(viewport.max.y - lead).max(0.0),
			);
			let content_top = ui.cursor().top();
			let clip = ui.clip_rect();
			// Measure leading overscan without changing the visible rows or parent bounds.
			// Its new heights take effect together with anchor restoration next pass.
			// The scroll content's box is only one viewport tall. A child that starts
			// there and then skips to `top` lays those rows past the box, so they
			// collapse and the live edge snaps when the real height comes back.
			let origin = ui.cursor().min;
			let mut leading = ui.new_child(
				egui::UiBuilder::new()
					.id_salt("leading-measurements")
					.max_rect(egui::Rect::from_min_size(
						origin,
						egui::vec2(
							ui.available_width(),
							(top + viewport.height() + overscan).max(ui.available_height()),
						),
					)),
			);
			leading.set_clip_rect(clip.with_max_y(clip.min.y));
			leading.add_space(top);
			ui.add_space(anchor_top);
			let keyboard_focus = ui
				.memory(|m| m.focused())
				.and_then(|id| ui.ctx().read_response(id));
			let retained_toolbar = self.toolbar.filter(|(_, rect)| {
				egui::Popup::is_any_open(ui.ctx())
					|| keyboard_focus
						.as_ref()
						.is_some_and(|r| rect.contains_rect(r.rect))
			});
			let reuse_leading = keyboard_focus.is_none()
				&& retained_toolbar.is_none()
				&& !egui::Popup::is_any_open(ui.ctx())
				&& !crate::select::has_selection(ui.ctx())
				&& !ui.input(|input| input.pointer.any_down() || input.pointer.any_released());
			let name_hover = self.name_hover.take();
			let mut end = first;
			for index in first..self.rows.len() {
				let row_id = ui.make_persistent_id(self.rows[index].0.0);
				let ui = if index < anchor {
					&mut leading
				} else {
					&mut *ui
				};
				if index >= anchor && ui.cursor().top() > content_top + viewport.max.y + 100.0 {
					break;
				}
				end = index + 1;
				let id = self.rows[index].0;
				if id == INTRO
					&& let Some(intro) = intro.as_ref()
				{
					let response = ui
						.scope_builder(egui::UiBuilder::new().scope_id(row_id), |ui| match intro {
							Intro::Direct(user) => {
								intro_action = dm_intro(ui, user, state, avatars, profile, width)
									.or(intro_action.take());
							}
							Intro::Channel { name, kind, post } => {
								channel_intro(ui, name, *kind, *post, width);
							}
							Intro::Group(id) => {
								if let Some(channel) = state.channel(*id) {
									group_intro(ui, channel, state, avatars, width);
								}
							}
						})
						.response;
					measurements.push((id, intro_key(intro, state), response.rect.height()));
					continue;
				}
				if Some(id) == self.starter_row
					&& let Some(starter) = state.thread_starter()
				{
					let response = ui
						.scope_builder(egui::UiBuilder::new().scope_id(row_id), |ui| {
							starter_row(ui, starter, state, avatars, width);
						})
						.response;
					measurements.push((
						id,
						row_height_key(
							starter,
							None,
							self.unread_boundary,
							state,
							state.timeline.is_deleted(id),
						),
						response.rect.height(),
					));
					continue;
				}
				let can_mark_read = state.can_mark_read(id);
				let can_mark_unread = state.can_mark_unread(id);
				let Some(message) = display_message(state, id) else {
					continue;
				};
				if !message.author.webhook
					&& self.visible_authors.len() < client_core::member_search::LIMIT
					&& !self.visible_authors.contains(&message.author.id)
				{
					self.visible_authors.push(message.author.id);
				}
				let previous = index
					.checked_sub(1)
					.and_then(|i| display_message(state, self.rows[i].0));
				let deleted = state.timeline.is_deleted(id);
				// ponytail: reuse only settled ordinary text; dynamic media, references,
				// spoilers and reactions need explicit layout invalidation before caching.
				if index < anchor
					&& reuse_leading
					&& self.measured_rows.contains(&id)
					&& message.kind == 0
					&& !message.unsupported
					&& !message.extra_content.any()
					&& message.reply_to.is_none()
					&& message.interaction.is_none()
					&& message.attachments.is_empty()
					&& message.embeds.is_empty()
					&& message.components.is_empty()
					&& !message.content.contains(['<', '|', '/'])
					&& state
						.reactions
						.display(message)
						.is_some_and(<[_]>::is_empty)
					&& state.interactions.pending.is_none()
					&& let Some(&(key, height)) = self.heights.get(&id)
					&& key
						== row_height_key(message, previous, self.unread_boundary, state, deleted)
				{
					ui.add_space(height);
					// Keep one result per row: visible height updates below zip by index.
					measurements.push((id, key, height));
					continue;
				}
				#[cfg(test)]
				if index < anchor {
					self.leading_rendered += 1;
				}

				let compact =
					self.compact_messages || grouped(previous, message, self.unread_boundary);
				let new_day = previous
					.is_none_or(|previous| timestamp(previous.id).date() != timestamp(id).date());
				let response = ui.scope_builder(egui::UiBuilder::new().scope_id(row_id), |ui| {
					if new_day {
						let date = timestamp(id);
						divider(
							ui,
							format!("{} {}, {}", date.month(), date.day(), date.year()),
							false,
							self.compact_messages,
						);
					}
					if self.unread_boundary == Some(id) {
						divider(ui, "New messages".into(), true, self.compact_messages);
					}
					let colors = crate::design::palette(ui);
					let background = ui.painter().add(egui::Shape::Noop);
					let mut time_rect = None;
					// A reaction claims its own right click: the row menu must stay closed.
					let mut reaction_menu = false;
					// A double-click away from text, links and media uses the selected reaction preset.
					let mut double_click = None;
					let row = egui::Frame::NONE
						.inner_margin(egui::Margin {
							left: 16,
							right: 16,
							top: if self.compact_messages {
								3
							} else if compact {
								1
							} else if message.reply_to.is_some() {
								0
							} else {
								group_gap(self.compact_messages) - 2
							},
							bottom: 1,
						})
						.show(ui, |ui| {
							let mut surface = crate::select::Surface::new(ui, "row");
							ui.spacing_mut().item_spacing =
								egui::vec2(16.0, if self.compact_messages { 2.0 } else { 4.0 });
							if let Some(interaction) = message
								.interaction
								.as_deref()
								.filter(|_| message.reply_to.is_none())
							{
								ui.horizontal(|ui| {
									ui.spacing_mut().interact_size.y = 18.0;
									ui.spacing_mut().item_spacing.x = 6.0;
									reference_spine(ui, &colors);
									let avatar =
										avatars.show(ui, &interaction.user, 16.0, state.demo);
									surface.keep(&avatar);
									let name = ui.add(
										egui::Label::new(
											RichText::new(&interaction.user.name)
												.size(13.0)
												.family(crate::design::semibold_family(ui.ctx()))
												.color(colors.text),
										)
										.truncate(),
									);
									surface.keep(&name);
									let used = ui.add(
										egui::Label::new(
											RichText::new(crate::i18n::translate(
												"timeline-command-used",
											))
											.size(13.0)
											.color(colors.muted),
										)
										.truncate(),
									);
									surface.keep(&used);
									let label = if interaction.command.is_empty() {
										crate::i18n::translate("timeline-command-a-command")
									} else {
										format!("/{}", interaction.command)
									};
									let command = egui::Frame::NONE
										.fill(colors.mention_bg)
										.corner_radius(3)
										.inner_margin(egui::Margin::symmetric(4, 0))
										.show(ui, |ui| {
											ui.add(
												egui::Label::new(
													RichText::new(label)
														.size(13.0)
														.family(crate::design::semibold_family(
															ui.ctx(),
														))
														.color(colors.mention_text),
												)
												.truncate(),
											)
										})
										.inner;
									surface.keep(&command);
								});
							}
							// A poll result's reference opens through its result card instead.
							if let Some(reply) = message.reply_to.filter(|_| message.kind != 46) {
								ui.horizontal(|ui| {
									ui.spacing_mut().interact_size.y = 18.0;
									ui.spacing_mut().item_spacing.x = 6.0;
									reference_spine(ui, &colors);
									// Reuse only loaded content; never fetch a thread while painting.
									if message.reply_deleted || state.timeline.is_deleted(reply) {
										let deleted = ui.add(
											egui::Label::new(
												RichText::new(crate::i18n::translate(
													"timeline-show-with-scroll-message-deleted",
												))
												.size(13.0)
												.italics()
												.color(colors.muted),
											)
											.truncate(),
										);
										surface.keep(&deleted);
									} else {
										ui.scope(|ui| {
											ui.visuals_mut().disabled_alpha = 1.0;
											ui.add_enabled_ui(
												state.can_open_reply_target(reply),
												|ui| {
													let mut preview =
														egui::text::LayoutJob::default();
													let mut preview_emojis = Vec::new();
													let mut attachment_icon = None;
													if let Some(original) =
														state.timeline.get(reply)
													{
														let reply_avatar = avatars.show(
															ui,
															&original.author,
															16.0,
															state.demo,
														);
														if reply_avatar.clicked() {
															self.request_reply_target(reply);
														}
														surface.keep(&reply_avatar);
														attachment_icon = (!original
															.attachments
															.is_empty())
														.then(|| {
															if original.attachments.iter().any(
																|attachment| {
																	attachment.is_image()
																		|| attachment.is_video()
																},
															) {
																crate::icons::Icon::Image
															} else {
																crate::icons::Icon::File
															}
														});
														preview.append(
															&format!(
																"@{}  ",
																state.message_author_name(original)
															),
															0.0,
															egui::TextFormat {
																font_id: egui::FontId::new(
																	13.0,
																	crate::design::semibold_family(
																		ui.ctx(),
																	),
																),
																color: colors.muted,
																..Default::default()
															},
														);
														if crate::embeds::has_spoilers(original) {
															preview.append(
																"Spoiler",
																0.0,
																egui::TextFormat {
																	font_id:
																		egui::FontId::proportional(
																			13.0,
																		),
																	color: colors.muted,
																	..Default::default()
																},
															);
														} else if attachment_icon.is_some()
															&& original.content.trim().is_empty()
														{
															preview.append(
																&crate::i18n::translate(
																	"timeline-show-with-scroll-click-to-see-attachment",
																),
																0.0,
																egui::TextFormat {
																	font_id:
																		egui::FontId::proportional(
																			13.0,
																		),
																	color: colors.muted,
																	italics: true,
																	..Default::default()
																},
															);
														} else {
															let source =
																crate::mentions::MentionSource {
																	state,
																	channel: original.channel,
																};
															let formatted = self
																.formatted
																.get(reply, &original.content);
															queue_missing_channel_reference(
																&mut self.channel_reference_load,
																formatted,
																state,
																u32::MAX,
															);
															preview_emojis = formatted
																.append_inline_preview(
																	&mut preview,
																	ui,
																	&original.mentions,
																	Some(&source),
																	crate::mentions::known_roles(
																		state,
																		original.channel,
																	),
																	&state.channels,
																);
														}
													} else {
														preview.append(
															"Earlier message · View original",
															0.0,
															egui::TextFormat {
																font_id: egui::FontId::proportional(
																	13.0,
																),
																color: colors.muted,
																..Default::default()
															},
														);
													}
													let accessible = Formatted::inline_preview_text(
														&preview,
														&preview_emojis,
													);
													// Keep room for the trailing attachment glyph when truncating.
													let icon_size = 16.0;
													let label_width = ui.available_width()
														- attachment_icon.map_or(0.0, |_| {
															icon_size + ui.spacing().item_spacing.x
														});
													let (
														preview_pos,
														preview_galley,
														reply_preview,
													) = ui.allocate_ui(
														egui::vec2(
															label_width.max(0.0),
															ui.available_height(),
														),
														|ui| {
															egui::Label::new(preview)
																.truncate()
																.sense(egui::Sense::click())
																.layout_in_ui(ui)
														},
													)
													.inner;
													if let Some(icon) = attachment_icon {
														crate::icons::inline(
															ui,
															icon,
															icon_size,
															colors.muted,
														);
													}
													reply_preview.widget_info(|| {
														egui::WidgetInfo::labeled(
															egui::Role::Link,
															ui.is_enabled(),
															&accessible,
														)
													});
													surface.embed(
														&reply_preview,
														preview_pos,
														preview_galley.clone(),
													);
													Formatted::paint_inline_preview_emojis(
														ui,
														preview_pos,
														&preview_galley,
														&preview_emojis,
													);
													let reply_preview = reply_preview
														.on_hover_cursor(
															egui::CursorIcon::PointingHand,
														)
														.on_hover_text(crate::i18n::translate(
															"timeline-show-with-scroll-view-original-message",
														))
														.on_disabled_hover_text(
															crate::i18n::translate(
																"timeline-show-with-scroll-wait-for-readable-current-message-history",
															),
														);
													surface.keep(&reply_preview);
													if reply_preview.clicked() {
														self.request_reply_target(reply);
													}
												},
											);
										});
									}
								});
							}
							let system = message.system_message();
							let author_colors = state.message_author_colors(message);
							let name_color = author_colors.map_or(colors.text_strong, |role| {
								crate::design::role_name_color(
									role.primary,
									colors.chat,
									colors.text_strong,
								)
							});
							let mut body_bottom = f32::NAN;
							// Hovering the avatar underlines the author, like hovering the name.
							let mut avatar_hot = false;
							// The starter's avatar and its thread card, joined by a gutter rule.
							let mut thread_gutter = None;
							let mut thread_card_rect = None;
							ui.horizontal_top(|ui| {
								if system.is_some() {
									let (gutter, _) = ui.allocate_exact_size(
										egui::vec2(40.0, MESSAGE_LINE),
										egui::Sense::hover(),
									);
									let (icon, tint) = system_icon(message.kind, &colors);
									crate::icons::paint(
										ui.painter(),
										icon,
										egui::Rect::from_center_size(
											gutter.center() + egui::vec2(0.0, 1.5),
											egui::Vec2::splat(18.0),
										),
										tint,
									);
								} else if self.compact_messages {
									// Compact rows drop the avatar gutter: time, author, then
									// the body in its own column so wrapped lines never run under the name.
									let body_spacing = ui.spacing().item_spacing.x;
									ui.spacing_mut().item_spacing.x = 8.0;
									let time = timestamp(id);
									compact_header(
										ui,
										ui.available_width(),
										egui::FontId::proportional(12.0),
										|ui| {
											let time = ui
												.label(
													RichText::new(crate::local_time::clock(time))
														.size(12.0)
														.color(colors.muted),
												)
												.on_hover_text_with(|| {
													crate::local_time::full(time)
												});
											surface.exclude(time.rect);
										},
									);
									let width = compact_author_width(ui.available_width());
									compact_header(
										ui,
										width,
										egui::FontId::new(
											15.5,
											crate::design::medium_family(ui.ctx()),
										),
										|ui| {
											ui.set_max_width(width);
											let author = crate::name_styles::in_row(
												name_hover == Some(message.id),
												|| {
													crate::account_badge::name(
														ui,
														&message.author,
														state.message_author_name(message),
														15.5,
														(
															colors.text_strong,
															author_colors,
															colors.chat,
														),
														egui::Sense::click(),
														0.0,
													)
												},
											)
											.on_hover_cursor(egui::CursorIcon::PointingHand);
											if author.hovered() {
												ui.painter().hline(
													author.rect.x_range(),
													author.rect.bottom() - 1.0,
													egui::Stroke::new(1.0, name_color),
												);
											}
											crate::user_menu::show(
												&author,
												state,
												&message.author,
												profile,
												&mut self.user_action,
											);
											profile.person_click(
												ui,
												&author,
												None,
												&message.author,
											);
											surface.keep(&author);
										},
									);
									ui.spacing_mut().item_spacing.x = body_spacing;
								} else if compact {
									time_rect = Some(
										ui.allocate_exact_size(
											egui::vec2(40.0, MESSAGE_LINE),
											egui::Sense::hover(),
										)
										.0,
									);
								} else {
									let avatar = avatars
										.show(ui, &message.author, 40.0, state.demo)
										.on_hover_cursor(egui::CursorIcon::PointingHand);
									avatar_hot = avatar.hovered();
									thread_gutter = Some(avatar.rect);
									crate::user_menu::show(
										&avatar,
										state,
										&message.author,
										profile,
										&mut self.user_action,
									);
									profile.person_click(ui, &avatar, None, &message.author);
									surface.keep(&avatar);
								}
								ui.vertical(|ui| {
									ui.set_width(ui.available_width());
									let mut text_line = egui::Rect::NOTHING;
									if !compact && system.is_none() {
										if message.reply_to.is_none() {
											ui.add_space(-2.0);
										}
										ui.allocate_ui_with_layout(
											egui::vec2(ui.available_width(), MESSAGE_LINE),
											egui::Layout::left_to_right(egui::Align::Center),
											|ui| {
												ui.spacing_mut().item_spacing.x = 8.0;
												let author = crate::name_styles::in_row(
													name_hover == Some(message.id),
													|| {
														crate::account_badge::name(
															ui,
															&message.author,
															state.message_author_name(message),
															15.5,
															(
																colors.text_strong,
																author_colors,
																colors.chat,
															),
															egui::Sense::click(),
															48.0,
														)
													},
												)
												.on_hover_cursor(egui::CursorIcon::PointingHand);
												if avatar_hot || author.hovered() {
													let line = author.rect.bottom() - 1.0;
													ui.painter().hline(
														author.rect.x_range(),
														line,
														egui::Stroke::new(1.0, name_color),
													);
												}
												crate::user_menu::show(
													&author,
													state,
													&message.author,
													profile,
													&mut self.user_action,
												);
												profile.person_click(
													ui,
													&author,
													None,
													&message.author,
												);
												surface.keep(&author);
												let time = timestamp(id);
												let time = ui
													.label(
														RichText::new(crate::local_time::header(
															time,
														))
														.size(12.0)
														.color(colors.muted),
													)
													.on_hover_text_with(|| {
														crate::local_time::full(time)
													});
												surface.exclude(time.rect);
											},
										);
									}
									if let Some(system) = &system {
										// Only a thread-start row carries the thread affordances.
										let thread = (message.kind == 18 && system.content_shown)
											.then(|| {
												(
													state.thread_of(message).map(|c| c.id),
													message.channel,
												)
											});
										show_system(
											ui,
											system,
											timestamp(id),
											state,
											profile,
											&mut self.user_action,
											&mut surface,
											thread,
											(
												&mut self.channel_reference,
												&mut self.threads_request,
											),
										);
									}
									if message.reply_to.is_some() {
										ui.add_space(0.6);
									} else if compact && !self.compact_messages {
										ui.add_space(1.4);
									}
									let body = egui::Frame::NONE
										.inner_margin(egui::Margin {
											left: if message.forwarded { 16 } else { 0 },
											..Default::default()
										})
										.show(ui, |ui| {
											if message.forwarded {
												ui.label(
													RichText::new(crate::i18n::translate(
														"timeline-show-with-scroll-forwarded",
													))
													.size(13.0)
													.italics()
													.color(colors.muted),
												);
												ui.add_space(4.0);
											}
											let body_color = if deleted
												&& !self.suppressed_deleted_highlight.contains(&id)
											{
												Some(colors.danger)
											} else {
												None
											};
											let formatted =
												self.formatted.get(id, &message.content);
											let reveal = self
												.revealed
												.get(&id)
												.filter(|reveal| reveal.matches(message));
											let before = reveal.map_or((0, false), |reveal| {
												(reveal.text, reveal.media)
											});
											let mut text =
												if formatted.spoilers { before.0 } else { 0 };
											queue_missing_channel_reference(
												&mut self.channel_reference_load,
												formatted,
												state,
												text,
											);
											let mut media = before.1;
											let content_shown =
												system.as_ref().is_some_and(|s| s.content_shown);
											if !content_shown
												&& !(self.hide_media_links
													&& crate::embeds::standalone_media_links(
														message,
													)) {
												let jumbo = formatted.jumbo();
												text_line = ui
													.scope(|ui| {
														if jumbo {
															crate::design::jumbo_emoji(ui);
														}
														if let Some(color) = body_color {
															ui.visuals_mut().override_text_color =
																Some(color);
														}
														if deleted && message.content.is_empty() {
															ui.label(
																RichText::new(
																	crate::i18n::translate(
																		"timeline-show-with-scroll-deleted-message-had-no-text",
																	),
																)
																.size(16.0)
																.color(
																	body_color
																		.unwrap_or(colors.text),
																),
															);
														} else {
															let source =
																crate::mentions::MentionSource {
																	state,
																	channel: message.channel,
																};
															formatted.show_references(
																ui,
																&mut self.opening,
																&message.mentions,
																Some(&source),
																profile,
																(
																	&state.channels,
																	&mut self.channel_reference,
																	&state.guilds,
																	crate::mentions::known_roles(
																		state,
																		message.channel,
																	),
																),
																(avatars, state.demo, &mut text),
																&mut surface,
																crate::design::MessageCardSurface::Conversation,
															);
														}
													})
													.response
													.rect;
											}
											if formatted.limited {
												ui.label(
													RichText::new(crate::i18n::translate(
														"timeline-show-with-scroll-display-limited-copy-message-for-the-full-text",
													))
													.small()
													.color(colors.muted),
												);
											}
											if crate::embeds::has_media_spoilers(message) && !media
											{
												let reveal = ui.button(crate::i18n::translate(
													"timeline-show-with-scroll-reveal-spoiler-media",
												));
												surface.keep(&reveal);
												if reveal.clicked() {
													media = true;
												}
											} else {
												if !message.extra_content.components_v2 {
													let previous_view = self.viewing;
													crate::attachments::show(
														ui,
														message,
														avatars,
														&mut self.viewing,
														&mut self.opening,
														&mut self.download,
														&mut self.audio,
														&mut self.video,
														state.demo,
														&mut surface,
														crate::design::MessageCardSurface::Conversation,
													);
													if self.viewing != previous_view {
														self.component_viewing = None;
														self.embed_viewing = None;
													}
												}
												let invite_top = ui.cursor().top();
												crate::invites::show(
													ui,
													message,
													state,
													avatars,
													&mut self.invite_requests,
													&mut self.invite_action,
												);
												surface.exclude(egui::Rect::from_min_max(
													egui::pos2(ui.max_rect().left(), invite_top),
													egui::pos2(
														ui.max_rect().right(),
														ui.min_rect().bottom(),
													),
												));
												let embed_top = ui.cursor().top();
												let poll =
													message.reply_to.filter(|_| message.kind == 46);
												if crate::embeds::poll_result(
													ui,
													message,
													poll.is_some_and(|id| {
														state.can_open_reply_target(id)
													}),
												) && let Some(poll) = poll
												{
													self.request_reply_target(poll);
												}
												if let Some(gif) = crate::embeds::show(
													ui,
													message,
													&mut self.formatted,
													avatars,
													&mut self.opening,
													&mut self.download,
													profile,
													state,
													crate::design::MessageCardSurface::Conversation,
													self.hide_media_links,
												) {
													self.gif_favorite = Some(gif);
												}
												surface.exclude(egui::Rect::from_min_max(
													egui::pos2(ui.max_rect().left(), embed_top),
													egui::pos2(
														ui.max_rect().right(),
														ui.min_rect().bottom(),
													),
												));
											}
											if before != (text, media) {
												if text == 0 && !media {
													self.revealed.remove(&id);
												} else {
													self.revealed.insert(
														id,
														Revealed::new(message, text, media),
													);
												}
												self.heights.remove(&id);
												ui.ctx().request_repaint();
											}
											if message.edited {
												ui.label(
													RichText::new(crate::i18n::translate(
														"timeline-show-with-scroll-edited",
													))
													.small()
													.color(colors.muted),
												);
											}
											if !message.components.is_empty() {
												let shown = ui.scope(|ui| {
													self.components.show(
														ui,
														message,
														state,
														avatars,
														&mut self.opening,
														&mut crate::components::MediaUi {
															component_viewing: &mut self
																.component_viewing,
															viewing: &mut self.viewing,
															download: &mut self.download,
															audio: &mut self.audio,
															video: &mut self.video,
														},
													)
												});
												surface.exclude(shown.response.rect);
												if shown.inner.is_some() {
													self.component_action = shown.inner;
												}
											}
											if state.interactions.pending.as_ref().is_some_and(
												|pending| pending.message == Some(message.id),
											) {
												ui.small(crate::i18n::translate(
													"timeline-show-with-scroll-application-interaction-pending",
												));
											}
											if !message.components.is_empty()
												&& let Some(error) = state.interactions.error
											{
												ui.colored_label(colors.danger, error);
											}
											if message.ephemeral {
												ui.horizontal(|ui| {
													ui.spacing_mut().item_spacing.x = 4.0;
													let (icon, _) = ui.allocate_exact_size(
														egui::Vec2::splat(16.0),
														egui::Sense::hover(),
													);
													crate::icons::paint(
														ui.painter(),
														crate::icons::Icon::EyeSlash,
														icon,
														colors.muted,
													);
													ui.label(
														RichText::new(crate::i18n::translate(
															"timeline-show-with-scroll-only-you-can-see-this",
														))
														.size(13.0)
														.color(colors.muted),
													);
													let dismiss = ui
														.add(
															egui::Button::new(
																RichText::new(
																	crate::i18n::translate(
																		"timeline-show-with-scroll-dismiss-message",
																	),
																)
																.size(13.0)
																.color(colors.link),
															)
															.frame(false),
														)
														.on_hover_cursor(
															egui::CursorIcon::PointingHand,
														);
													surface.keep(&dismiss);
													if dismiss.clicked() {
														self.dismiss_ephemeral = Some(id);
													}
												});
											}

											for sticker in &message.sticker_items {
												let response = ui
													.push_id(sticker.id, |ui| {
														crate::stickers::message(
															ui,
															sticker,
															state,
															avatars,
															&mut self.sticker_request,
															&mut self.browse_sticker,
														)
													})
													.inner;
												surface.keep(&response);
											}
											if message.poll.is_some() {
												let shown = ui.scope(|ui| {
													self.polls.show(ui, state, message, avatars)
												});
												surface.exclude(shown.response.rect);
												if let Some(action) = shown.inner {
													self.poll_action = Some((message.id, action));
												}
											}
											let unknown_system = message.unsupported
												&& message.system_summary().is_none();
											if unknown_system
												|| (message.extra_content.poll
													&& message.poll.is_none()) || ((message
												.extra_content
												.sticker_items
												|| message.extra_content.stickers)
												&& message.sticker_items.is_empty()) || ((message
												.extra_content
												.components
												|| message.extra_content.components_v2)
												&& message.components.is_empty())
											{
												if unknown_system {
													ui.label(
														RichText::new(format!(
															"Unsupported message type {} · Preview unavailable",
															message.kind
														))
														.small()
														.color(colors.muted),
													);
												}
												for (present, label) in [
													(
														message.extra_content.poll
															&& message.poll.is_none(),
														"Poll · Preview unavailable",
													),
													(
														(message.extra_content.sticker_items
															|| message.extra_content.stickers)
															&& message.sticker_items.is_empty(),
														"Sticker · Preview unavailable",
													),
													(
														(message.extra_content.components
															|| message.extra_content.components_v2)
															&& message.components.is_empty(),
														"Components · Preview unavailable",
													),
												] {
													if present {
														ui.label(
															RichText::new(label)
																.small()
																.color(colors.muted),
														);
													}
												}
												let target = state
													.channels
													.iter()
													.find(|c| {
														c.id == message.channel
															&& state.can_view(c.id)
													})
													.and_then(|c| discord_url(c, Some(message.id)));
												let open = ui.add_enabled(
													target.is_some(),
													egui::Button::new(crate::i18n::translate(
														"timeline-show-with-scroll-open-in-discord",
													)),
												);
												surface.keep(&open);
												if open.clicked() {
													self.browser_opening = target;
												}
											}
										});
									body_bottom = body.response.rect.bottom();
									if message.forwarded {
										let rail = egui::Rect::from_min_max(
											body.response.rect.min,
											egui::pos2(
												body.response.rect.left() + 3.0,
												body.response.rect.bottom(),
											),
										);
										ui.painter().rect_filled(rail, 2.0, colors.selected);
									}
									// A "started a thread" row already links the thread inline.
									if system.is_none()
										&& let Some(thread) = state
											.thread_of(message)
											.map(|thread| thread.id)
											.or_else(|| state.unloaded_thread(message))
									{
										// The name and count come from the thread's channel; its
										// latest message only exists once the bounded read lands.
										let channel = state.channel(thread);
										let preview = state.thread_preview(thread);
										let name = channel
											.map(|channel| channel.name.as_str())
											.or_else(|| {
												preview.as_ref().and_then(|preview| {
													preview
														.channel
														.map(|channel| channel.name.as_str())
												})
											});
										let count = channel
											.and_then(|channel| channel.message_count)
											.or_else(|| {
												preview.as_ref().and_then(|preview| {
													preview
														.channel
														.and_then(|channel| channel.message_count)
												})
											});
										let last =
											preview.as_ref().and_then(|preview| preview.last);
										let text = last.map(|reply| {
											thread_preview_text(
												ui,
												state,
												&mut self.formatted,
												message.channel,
												reply,
											)
										});
										let latest = last
											.map(|message| message.id)
											.or_else(|| {
												channel.and_then(|channel| channel.last_message)
											})
											.or_else(|| {
												preview.as_ref().and_then(|preview| {
													preview
														.channel
														.and_then(|channel| channel.last_message)
												})
											});
										let (card, visible) = thread_card(
											ui,
											state,
											avatars,
											ThreadCard {
												name,
												count,
												last,
												text: text.as_deref(),
												latest,
											},
											&colors,
										);
										surface.keep(&card);
										if card.clicked() {
											self.channel_reference = Some(thread);
										}
										thread_card_rect = Some(card.rect);
										if visible && state.thread_preview_needs_read(thread) {
											self.thread_preview_request.get_or_insert(message.id);
										}
									}
									let reactions_top = ui.cursor().top();
									if deleted {
										crate::reactions::show_frozen(
											ui,
											state.reactions.display(message),
											(avatars, state.demo),
										);
									} else if let Some(action) = crate::reactions::show(
										ui,
										state.reactions.display(message),
										state.gateway_connected
											&& state.freshness == model::Freshness::Fresh
											&& state.can_read_history(message.channel),
										state.reactions.busy(),
										(state.history_pending && state.history_before.is_none())
											|| state.reactions.invalidated(message.id),
										(avatars, state.demo),
										message.id,
										state.reactions.users.as_ref(),
										|emoji, add| state.can_react(id, Some(emoji), add),
									) {
										match action {
											crate::reactions::Action::Reload => {
												self.reaction = Some((id, None));
											}
											crate::reactions::Action::Toggle(emoji) => {
												self.reaction = Some((id, Some(emoji)));
											}
											crate::reactions::Action::Inspect(emoji, open) => {
												reaction_menu |= open;
												self.reaction_users = Some((id, emoji, open));
											}
										}
									}
									surface.exclude(egui::Rect::from_min_max(
										egui::pos2(ui.max_rect().left(), reactions_top),
										egui::pos2(ui.max_rect().right(), ui.min_rect().bottom()),
									));
									fill_header_line(ui, compact, text_line);
								});
							});
							// Discord ties the starter's avatar to the thread card under it.
							if let (Some(avatar), Some(card)) = (thread_gutter, thread_card_rect) {
								thread_connector(ui, avatar, card, &colors);
							}
							let mut cover = ui.min_rect();
							if body_bottom.is_finite() {
								cover.max.y = body_bottom;
							}
							double_click = ui
								.input(|input| {
									input
										.pointer
										.button_double_clicked(egui::PointerButton::Primary)
										.then(|| input.pointer.interact_pos())
										.flatten()
								})
								.filter(|pos| surface.blank_at(*pos));
							surface.cover(cover);
							surface.finish(ui);
						});
					let rect = row.response.rect;
					if let Some(pos) = double_click
						&& let Some(text) = self.double_click_reaction
						&& !deleted && rect.contains(pos)
						&& self
							.toolbar
							.is_none_or(|(_, toolbar)| !toolbar.contains(pos))
						&& ui.ctx().layer_id_at(pos) == Some(ui.layer_id())
						&& !egui::Popup::is_any_open(ui.ctx())
					{
						let (emoji, reacted, enabled) = quick_reaction(state, message, text);
						if enabled {
							// The second click selected the nearest word in the blank band.
							crate::select::clear(ui.ctx());
							self.reaction = Some((id, Some(emoji)));
							if !reacted {
								self.quick_reaction_used = Some(text);
							}
						}
					}
					let mentioned = mentions_viewer(message, state);
					// Mentions mark the row in the warning colour; a private command response in
					// the house accent, as in the official client.
					let marked = if mentioned {
						Some(colors.warning)
					} else if message.ephemeral {
						Some(colors.accent)
					} else {
						None
					};
					if let Some(mark) = marked {
						ui.painter().set(
							background,
							egui::Shape::rect_filled(
								rect,
								0.0,
								crate::design::row_highlight(ui, mark, 0.10),
							),
						);
						ui.painter().rect_filled(
							egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height())),
							0.0,
							mark,
						);
					}
					let focus = ui.interact(
						rect,
						ui.scope_id().with("message-focus"),
						egui::Sense::focusable_noninteractive(),
					);
					focus.widget_info(|| {
						egui::WidgetInfo::labeled(
							egui::Role::Label,
							true,
							format!(
								"Message by {}. {}Tab for actions.",
								message.author.name,
								if mentioned { "Mentions you. " } else { "" },
							),
						)
					});
					let retained = retained_toolbar.is_some_and(|(active, _)| active == id);
					// The floating toolbar overlaps the row above; pointer inside it keeps this row active.
					let toolbar_hover = self
						.toolbar
						.filter(|(active, toolbar)| {
							*active == id && ui.rect_contains_pointer(*toolbar)
						})
						.is_some();
					let other_toolbar_hover = self
						.toolbar
						.filter(|(active, toolbar)| {
							*active != id && ui.rect_contains_pointer(*toolbar)
						})
						.is_some();
					let hovered = allow_hover
						&& (ui.rect_contains_pointer(rect) || toolbar_hover)
						&& !other_toolbar_hover
						&& !egui::Popup::is_any_open(ui.ctx())
						&& retained_toolbar.is_none_or(|(active, _)| active == id);
					if hovered {
						self.name_hover = Some(id);
					}
					let context_menu = (ui.rect_contains_pointer(rect) || toolbar_hover)
						&& !other_toolbar_hover
						&& !reaction_menu && !egui::Popup::is_any_open(ui.ctx())
						&& (ui.input(|i| i.pointer.secondary_clicked())
							|| crate::select::open_menu(ui.ctx()));
					if context_menu
						|| hovered || focus.has_focus()
						|| keyboard_focus.as_ref().is_some_and(|r| r.id == focus.id)
						|| retained
					{
						ui.painter().set(
							background,
							egui::Shape::rect_filled(
								rect,
								0.0,
								if let Some(mark) = marked {
									crate::design::row_highlight(ui, mark, 0.16)
								} else {
									crate::design::row_highlight(ui, colors.hover, 0.7)
								},
							),
						);
						if let Some(rect) = time_rect {
							let time = timestamp(id);
							ui.painter().text(
								rect.center(),
								egui::Align2::CENTER_CENTER,
								crate::local_time::clock(time),
								egui::FontId::proportional(11.0),
								colors.muted,
							);
							ui.interact(
								rect,
								ui.scope_id().with("timestamp"),
								egui::Sense::hover(),
							)
							.on_hover_text_with(|| crate::local_time::full(time));
						}
						let own = state
							.user
							.as_ref()
							.is_some_and(|u| u.id == message.author.id);
						if deleted {
							let toolbar_rect = egui::Rect::from_min_size(
								egui::pos2(rect.right() - 46.0, rect.top() - 10.0),
								egui::vec2(36.0, 28.0),
							);
							let mut toolbar = ui.new_child(
								egui::UiBuilder::new()
									.id_salt("hover-actions")
									.max_rect(toolbar_rect)
									.layout(egui::Layout::left_to_right(egui::Align::Center)),
							);
							toolbar.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
							toolbar.spacing_mut().button_padding = egui::vec2(4.0, 2.0);
							toolbar.spacing_mut().interact_size.y = 28.0;
							toolbar
								.painter()
								.rect_filled(toolbar_rect, 6.0, colors.raised);
							toolbar.painter().rect_stroke(
								toolbar_rect,
								6.0,
								egui::Stroke::new(1.0, colors.border),
								egui::StrokeKind::Inside,
							);
							let menu = action_button(
								&mut toolbar,
								crate::icons::Icon::More,
								"profiles-show-more",
							);
							menu.widget_info(|| {
								egui::WidgetInfo::labeled(
									egui::Role::Button,
									toolbar.is_enabled(),
									crate::i18n::translate_args(
										"timeline-deleted-message-actions-for",
										&[("user", &message.author.name)],
									),
								)
							});
							let mut popup = egui::Popup::menu(&menu);
							if context_menu {
								popup = popup.open_memory(Some(egui::SetOpenCommand::Bool(true)));
							}
							if context_menu
								|| (!menu.clicked()
									&& egui::Popup::position_of_id(toolbar.ctx(), popup.get_id())
										.is_some())
							{
								popup = popup.at_pointer_fixed();
							}
							let mut action = None;
							deleted_message_actions(popup, &mut action);
							match action {
								Some(DeletedLocalAction::ToggleHighlight) => {
									if !self.suppressed_deleted_highlight.remove(&id) {
										self.suppressed_deleted_highlight.insert(id);
									}
								}
								Some(DeletedLocalAction::Remove) => {
									self.remove_preserved = Some(id);
								}
								None => {}
							}
							self.toolbar = Some((id, toolbar_rect));
						} else {
							let react = state.can_react(id, None, true)
								|| message.reactions.as_ref().is_some_and(|items| {
									items
										.iter()
										.any(|r| state.can_react(id, Some(&r.emoji), true))
								});
							// Holding Shift exposes direct actions alongside the menu.
							let shift = ui.input(|input| input.modifiers.shift);
							let can_pin = state.can_pin(message.channel, id);
							let can_delete = state.can_delete(message.channel, id);
							let link = state
								.channel(message.channel)
								.and_then(|channel| discord_url(channel, Some(id)));
							let buttons = if react && !shift { 4 } else { 1 }
								+ 3 + usize::from(own) + if shift {
								2 + usize::from(link.is_some())
									+ usize::from(can_delete) + usize::from(can_mark_unread)
									+ usize::from(can_pin)
							} else {
								0
							};
							let width = buttons as f32 * 30.0;
							let toolbar_rect = egui::Rect::from_min_size(
								egui::pos2(
									(rect.right() - 16.0 - width).max(rect.left()),
									rect.top() - 10.0,
								),
								egui::vec2(width, 28.0),
							);
							// A child overlay keeps hover from changing wrapping or cached row heights.
							let mut toolbar = ui.new_child(
								egui::UiBuilder::new()
									.id_salt("hover-actions")
									.max_rect(toolbar_rect)
									.layout(egui::Layout::left_to_right(egui::Align::Center)),
							);
							toolbar.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
							toolbar.spacing_mut().button_padding = egui::vec2(4.0, 2.0);
							toolbar.spacing_mut().interact_size.y = 28.0;
							toolbar
								.painter()
								.rect_filled(toolbar_rect, 6.0, colors.raised);
							toolbar.painter().rect_stroke(
								toolbar_rect,
								6.0,
								egui::Stroke::new(1.0, colors.border),
								egui::StrokeKind::Inside,
							);
							if react && !shift {
								let quick = self
									.quick_reactions
									.unwrap_or(crate::reactions::QUICK_DEFAULTS);
								let mut last = egui::Rect::NOTHING;
								for (index, text) in quick.into_iter().enumerate() {
									let (emoji, reacted, enabled) =
										quick_reaction(state, message, text);
									let response = toolbar
										.push_id(("quick-reaction", index), |ui| {
											ui.add_enabled_ui(enabled, |ui| {
												crate::reactions::quick_button(ui, text, reacted)
											})
											.inner
										})
										.inner;
									last = response.rect;
									if response.clicked() {
										self.reaction = Some((id, Some(emoji)));
										if !reacted {
											self.quick_reaction_used = Some(text);
										}
									}
								}
								let x = last.right() + 1.0;
								toolbar.painter().vline(
									x,
									last.y_range().shrink(6.0),
									egui::Stroke::new(1.0, colors.border),
								);
							}
							if let Some((anchor, trigger)) = crate::reactions::add_button(
								&mut toolbar,
								react,
								state.reactions.busy(),
							) {
								self.reaction_picker = Some((id, anchor, trigger));
							}
							let can_reply = state.can_send(message.channel) && !message.ephemeral;
							let can_edit =
								!message.unsupported && state.can_edit(message.channel, id);
							if toolbar
								.add_enabled_ui(can_reply, |ui| {
									action_button(
										ui,
										crate::icons::Icon::Reply,
										"timeline-message-actions-reply",
									)
								})
								.inner
								.clicked()
							{
								selected_reply = Some(id);
							}
							if toolbar
								.add_enabled_ui(state.can_forward(id), |ui| {
									action_button(
										ui,
										crate::icons::Icon::Forward,
										"timeline-message-actions-forward",
									)
								})
								.inner
								.clicked()
							{
								self.forward_request = Some(id);
							}
							if own
								&& toolbar
									.add_enabled_ui(can_edit, |ui| {
										action_button(
											ui,
											crate::icons::Icon::Pencil,
											"timeline-message-actions-edit-message",
										)
									})
									.inner
									.clicked()
							{
								*editing = Some((message.channel, id, message.content.clone()));
								self.edit_started = true;
							}
							if shift {
								let pinned = state.is_pinned(message.channel, id);
								if can_pin
									&& action_button(
										&mut toolbar,
										crate::icons::Icon::Pin,
										if pinned {
											"timeline-message-actions-unpin-message"
										} else {
											"timeline-message-actions-pin-message"
										},
									)
									.clicked()
								{
									self.pin_request = Some((message.channel, id, !pinned));
								}
								if can_mark_unread
									&& action_button(
										&mut toolbar,
										crate::icons::Icon::Inbox,
										"timeline-message-actions-mark-unread",
									)
									.clicked()
								{
									self.mark_unread = Some(id);
								}
								if let Some(link) = &link {
									copy_action_button(
										&mut toolbar,
										crate::icons::Icon::Link,
										"timeline-message-actions-copy-message-link",
										egui::Id::unique(("copy-link", state.generation, id)),
										&mut self.copy_feedback,
										|| link.clone(),
									);
								}
								copy_action_button(
									&mut toolbar,
									crate::icons::Icon::Copy,
									"message-menu-copy",
									egui::Id::unique(("copy-message", state.generation, id)),
									&mut self.copy_feedback,
									|| crate::i18n::message_text(message).into_owned(),
								);
								copy_action_button(
									&mut toolbar,
									crate::icons::Icon::Hash,
									"timeline-message-actions-copy-message-id",
									egui::Id::unique(("copy-id", state.generation, id)),
									&mut self.copy_feedback,
									|| id.to_string(),
								);
							}
							if can_delete
								&& shift && toolbar
								.push_id("quick-delete", |ui| {
									action_button(
										ui,
										crate::icons::Icon::Trash,
										"timeline-message-actions-delete-message",
									)
								})
								.inner
								.clicked()
							{
								self.quick_delete = Some((message.channel, id));
							}
							let menu = action_button(
								&mut toolbar,
								crate::icons::Icon::More,
								"profiles-show-more",
							);
							menu.widget_info(|| {
								egui::WidgetInfo::labeled(
									egui::Role::Button,
									toolbar.is_enabled(),
									crate::i18n::translate_args(
										"timeline-message-actions-for",
										&[("user", &message.author.name)],
									),
								)
							});
							let mut popup = egui::Popup::menu(&menu).id(egui::Id::unique((
								"message-menu",
								state.generation,
								id,
							)));
							if context_menu {
								popup = popup.open_memory(Some(egui::SetOpenCommand::Bool(true)));
							}
							if context_menu
								|| (!menu.clicked()
									&& egui::Popup::position_of_id(toolbar.ctx(), popup.get_id())
										.is_some())
							{
								popup = popup.at_pointer_fixed();
							}
							message_actions(
								popup,
								(
									message,
									link.as_deref(),
									&self.extension_actions,
									&mut self.extension_request,
								),
								(own, can_reply, can_edit, can_delete),
								(
									can_mark_read.then_some(&mut self.mark_read),
									can_mark_unread.then_some(&mut self.mark_unread),
									&mut selected_reply,
								),
								(editing, &mut self.edit_started),
								deleting,
								(
									can_pin,
									state.is_pinned(message.channel, id),
									&mut self.pin_request,
								),
								(
									state.can_create_thread(message.channel)
										&& state.thread_of(message).is_none(),
									&mut self.thread_request,
								),
								(state.can_forward(id), &mut self.forward_request),
								message
									.reactions
									.as_ref()
									.filter(|_| state.can_read_history(message.channel))
									.and_then(|items| items.first())
									.map(|reaction| {
										(reaction.emoji.clone(), &mut self.reaction_users)
									}),
							);
							self.toolbar = Some((id, toolbar_rect));
						}
					}
					if selected_reply.or(state.reply_target()) == Some(id)
						|| self.highlighted.is_some_and(|(target, _)| target == id)
					{
						ui.painter().set(
							background,
							egui::Shape::rect_filled(
								rect,
								0.0,
								crate::design::row_highlight(ui, colors.accent, 0.25),
							),
						);
					}
				});
				measurements.push((
					id,
					row_height_key(message, previous, self.unread_boundary, state, deleted),
					response.response.rect.height(),
				));
			}
			// Names follow the hovered row a pass late; draw that pass even if the pointer stops.
			if self.name_hover != name_hover {
				ui.ctx().request_repaint();
			}
			let used: f32 = self.rows[..end].iter().map(|(_, height)| *height).sum();
			ui.add_space((total - used).max(0.0));
			for (index, (pending, height)) in pending_rows.iter().enumerate() {
				let compact = index > 0
					|| state.timeline.iter().next_back().is_some_and(|previous| {
						let now = crate::local_time::now();
						state
							.user
							.as_ref()
							.is_some_and(|user| user.id == previous.author.id)
							&& !previous.unsupported
							&& !previous.extra_content.any()
							&& timestamp(previous.id).date() == now.date()
							&& (now - timestamp(previous.id)).whole_seconds() < 300
					});
				let top = ui.cursor().top() - content_top;
				if top + height < viewport.min.y - 100.0 || top > viewport.max.y + 100.0 {
					ui.add_space(*height);
					continue;
				}
				let response = ui.push_id(("pending", &pending.nonce), |ui| {
					crate::pending::show(
						ui,
						pending,
						(
							compact,
							group_gap(self.compact_messages),
							self.compact_messages,
						),
						state,
						(
							avatars,
							&mut self.opening,
							profile,
							&mut self.channel_reference,
							&mut self.pending_formatted,
						),
						upload,
						(
							&mut self.restore_pending,
							&mut self.dismiss_pending,
							&mut self.cancel_upload,
						),
					);
				});
				let measured = response.response.rect.height();
				if (measured - height).abs() > 1.0 {
					ui.ctx().request_discard("Pending message height settled");
					ui.ctx().request_repaint();
				}
				self.pending_heights.insert(pending.nonce.clone(), measured);
			}
			// Only the end of the conversation has extra space; it scrolls with the messages.
			ui.add_space(end_padding);
			// Visible rows occupy their measured height immediately; leading overscan
			// still occupies its old height until the next anchored pass.
			for (index, (_, _, height)) in (first..end).zip(&measurements) {
				if index >= anchor {
					self.rows[index].1 = *height;
				}
			}
			viewport.min.y
		});
		self.scroll_offset = output.state.clamped_offset().y;
		if jumped_to.is_some_and(|target| (self.scroll_offset - target).abs() > 1.0) {
			self.jump = false;
			ui.ctx().request_discard("Timeline live edge settled");
		}
		// ScrollArea applies wheel input after laying out its contents. Preserve that
		// movement when new row measurements rebuild the timeline on the next pass.
		let spare = if welcome || !live_edge {
			0.0
		} else {
			(output.inner_rect.height() - lead_packed).max(0.0)
		};
		let lead = spare;
		let (anchor, _, anchor_top) = visible_range(
			&self.rows,
			(output.state.clamped_offset().y - lead).max(0.0),
			(output.state.clamped_offset().y + output.inner_rect.height() - lead).max(0.0),
		);
		let inset = output.state.clamped_offset().y - lead - anchor_top;
		// The introduction is not a message, so a saved reading cursor could not
		// restore it; anchor on the first message with the introduction as negative inset.
		self.anchor = match self.rows.get(anchor) {
			Some(&(INTRO, height)) => self
				.rows
				.get(anchor + 1)
				.map(|(id, _)| (*id, inset - height)),
			row => row.map(|(id, _)| (*id, inset)),
		};
		if selected_reply.is_some() {
			state.reply = selected_reply.map(client_core::Reply::to);
			self.reply_started = true;
		}
		let distance_from_bottom =
			(output.content_size.y - output.state.clamped_offset().y - output.inner_rect.height())
				.max(0.0);
		let whole_conversation_visible =
			state.older_exhausted && packed <= output.inner_rect.height() + 3.0;
		let at_bottom = distance_from_bottom <= 3.0 || whole_conversation_visible;
		// The live edge counts even when service latest metadata outlived a deleted message;
		// otherwise the unread banners could never resolve for that channel.
		self.at_current_latest = state.live_edge_latest().is_some()
			|| state.timeline.iter().next_back().is_some_and(|message| {
				state.channels.iter().any(|channel| {
					Some(channel.id) == state.selected && channel.last_message == Some(message.id)
				})
			});
		let channel_latest = state
			.selected
			.and_then(|channel| state.channel(channel))
			.and_then(|channel| channel.last_message);
		let scrolled_toward_bottom = ui.input(|input| {
			(scroll_delta < 0.0
				&& (session.holding()
					|| input
						.pointer
						.hover_pos()
						.is_some_and(|pos| output.inner_rect.contains(pos))))
				|| (input.pointer.any_down() && output.state.clamped_offset().y > output.inner)
		});
		// A window that does not fill the view cannot be scrolled, so it fetches newer pages
		// until it does, like Discord after jumping to an early message.
		let underfilled = output.content_size.y <= output.inner_rect.height() + 1.0;
		if at_bottom && (can_load_newer || self.at_current_latest) {
			if can_load_newer {
				if underfilled && !scrolled_toward_bottom {
					self.fill_newer = true;
				} else if scrolled_toward_bottom {
					self.load_newer = true;
					self.browse_away();
					if autoscroll_delta == 0.0 {
						ui.ctx().request_repaint();
					}
				}
			} else if scrolled_toward_bottom {
				self.target_browsing = false;
				self.hold_read_ack = false;
				// Scrolling down to the live edge reads the section, so its banner goes away.
				self.unread_dismissed = Some(channel_latest);
				if state.history_targeted || state.history_after.is_some() {
					self.latest = true;
				}
				if autoscroll_delta == 0.0 {
					ui.ctx().request_repaint();
				}
			}
		}
		let was_following = self.following;
		self.following = at_bottom && !self.target_browsing;
		if self.following
			&& self.at_current_latest
			&& !state.history_targeted
			&& state.history_after.is_none()
			&& ui.input(|i| i.focused)
			&& let Some(latest) = state.live_edge_latest()
		{
			self.seen_latest = Some(latest);
			// A focused reader at the live edge reads each arrival as it lands: nothing they
			// watched may raise the banner or add to its count. An unacknowledged join keeps
			// its banner until the reader scrolls toward the bottom or marks it read.
			if !self.hold_read_ack && ui.is_enabled() {
				self.unread_dismissed = Some(channel_latest);
			}
		}
		if self.following
			&& !self.hold_read_ack
			&& ui.is_enabled()
			&& self.mark_unread.is_none()
			&& !state.history_targeted
			&& state.history_after.is_none()
			&& ui.input(|i| i.focused)
			&& let Some(latest) = state.live_edge_latest()
			&& self.auto_read_attempt != Some(latest)
			&& self.at_current_latest
			&& state.can_mark_read(latest)
		{
			// One automatic attempt per viewed latest message; failed ACKs remain manually retryable.
			self.auto_read_attempt = Some(latest);
			self.mark_read = Some(latest);
		}
		let mut reflow = false;
		if let Some(action) = intro_action {
			self.user_action = Some(action);
		}
		for (id, key, height) in measurements {
			self.measured_rows.insert(id);
			if self
				.heights
				.get(&id)
				.is_none_or(|(old_key, old)| *old_key != key || (*old - height).abs() > 1.0)
			{
				self.heights.insert(id, (key, height));
				reflow = true;
			}
		}
		if reflow {
			self.reflow_frames = self.reflow_frames.saturating_add(1);
			self.consecutive_reflows = self.consecutive_reflows.saturating_add(1);
			self.revision = u64::MAX;
			let user_scrolling = scroll_delta != 0.0 || session.holding();
			if was_following && !user_scrolling {
				self.following = true;
				self.jump = true;
				// An anchored reader keeps this frame's places. The next frame
				// applies the new leading height through the scroll anchor.
				if !dimensions_changed || channel_changed {
					ui.ctx().request_discard("Timeline message heights settled");
				}
			}
			ui.ctx().request_repaint();
		}
		if !reflow {
			self.consecutive_reflows = 0;
		}
		// Explicit upward input requests one page even when a short view cannot scroll.
		// Idle layout still never drains history merely to fill the viewport.
		self.load_older = output.state.clamped_offset().y < 160.0
			&& ui.input(|i| {
				scroll_delta > 0.0
					&& (session.holding()
						|| i.pointer
							.hover_pos()
							.is_some_and(|pos| output.inner_rect.contains(pos)))
			}) && state.can_load_older();
		if self.load_older {
			self.browse_away();
		}
		// Discord-style overlays: an unread strip hangs from the top edge, the typing indicator
		// floats in the reserved strip above the composer, and a round control offers the way
		// back to the live edge. They are painted after the scroll area so they sit above the
		// messages and win the hit-test.
		let area = output.inner_rect;
		let now = std::time::Instant::now();
		let typing = state
			.selected
			.filter(|channel| crate::typing::active(state, *channel, now));
		// The bottom edge fades into the composer: one continuous ramp from nothing down to the
		// chat surface, with no flat band anywhere in it. While someone is typing the ramp runs
		// tall enough to carry the indicator, and denser once the reader has scrolled away from
		// the live edge, so the line stays legible over the messages behind it.
		//
		// A see-through surface (window transparency or a background image) cannot hide messages
		// without also tinting what shows through, so its ramp only ever reaches the surface's
		// own tint, and at the live edge, where the reserved strip is empty, it is skipped.
		let surface = crate::design::section_surface(
			ui,
			crate::design::window_palette(ui).chat,
			crate::design::ImageSection::MessageList,
		);
		let see_through = surface.a() < 255;
		let fade_height = if see_through && self.following {
			0.0
		} else if typing.is_some() {
			crate::typing::OVERLAY_HEIGHT + 8.0
		} else {
			20.0
		};
		let dense = if see_through {
			surface
		} else if self.following {
			surface.gamma_multiply(0.88)
		} else {
			surface.to_opaque()
		};
		let fade_rect = egui::Rect::from_min_max(
			egui::pos2(area.left(), (area.bottom() - fade_height).max(area.top())),
			area.right_bottom(),
		);
		let mut fade = egui::Mesh::default();
		// Stacked strips approximate a smooth ease instead of a straight ramp, which reads as a
		// visible edge where it starts.
		const FADE_STEPS: usize = 12;
		for step in 0..=FADE_STEPS {
			let t = step as f32 / FADE_STEPS as f32;
			let y = fade_rect.top() + fade_rect.height() * t;
			let color = dense.gamma_multiply(t * t * (3.0 - 2.0 * t));
			fade.colored_vertex(egui::pos2(fade_rect.left(), y), color);
			fade.colored_vertex(egui::pos2(fade_rect.right(), y), color);
			if step > 0 {
				let base = (step as u32 - 1) * 2;
				fade.add_triangle(base, base + 1, base + 3);
				fade.add_triangle(base, base + 3, base + 2);
			}
		}
		ui.painter()
			.with_clip_rect(area)
			.add(egui::Shape::mesh(fade));
		if let Some(channel) = typing {
			crate::typing::overlay(
				ui,
				egui::Rect::from_min_max(
					egui::pos2(
						area.left() + 16.0,
						(area.bottom() - crate::typing::OVERLAY_HEIGHT).max(area.top()),
					),
					egui::pos2(area.right() - 16.0, area.bottom()),
				),
				state,
				channel,
				now,
			);
		}
		let browsing_history = state.history_targeted
			|| state.history_before.is_some()
			|| state.history_after.is_some();
		let missed = state.show_missed_banner();
		let opening_unread =
			missed && state.freshness == model::Freshness::Loading && state.history_pending;
		let raise_unread = missed
			&& (state.timeline.iter().next().is_some() || opening_unread)
			&& (opening_unread
				|| self.hold_read_ack
				|| !(self.following
					&& self.at_current_latest
					&& state.live_edge_latest().is_some()
					&& ui.input(|input| input.focused)));
		// Once raised, the banner stays with its divider for the rest of the visit, unless the
		// reader dismissed it and nothing newer has arrived since.
		let dismissed = self.unread_dismissed == Some(channel_latest);
		let kept_unread = self.unread_session && self.unread_boundary.is_some() && !dismissed;
		let show_unread = (raise_unread || kept_unread) && !dismissed;
		let can_jump_unread = show_unread && (state.can_jump_unread() || kept_unread);
		// Latest-message metadata can outlive a deleted message. A complete, visible
		// latest page has nowhere useful to jump; targeted pages still need navigation.
		// Older history needs no bar of its own: the round control leads back to the present.
		if show_unread
			&& (!whole_conversation_visible
				|| browsing_history
				|| opening_unread
				|| self.hold_read_ack
				|| kept_unread)
		{
			self.unread_session |= self.unread_boundary.is_some();
			let summary = self.unread_summary(state);
			let (jump_unread, mark_read) =
				unread_banner(ui, banner_rect(area), can_jump_unread, summary.as_deref());
			if jump_unread {
				if state.can_jump_unread() {
					self.unread_jump = true;
					self.browse_away();
				} else if let Some(boundary) = self.unread_boundary {
					// The section was acknowledged during this visit; its divider is still loaded.
					self.request_reply_target(boundary);
				}
				// The reader is at the section now; only later arrivals bring the banner back.
				self.unread_dismissed = Some(channel_latest);
			}
			if mark_read {
				self.unread_dismissed = Some(channel_latest);
				self.hold_read_ack = false;
				self.mark_channel_read = state.selected;
			}
		}
		// Older pages appended to the live timeline keep their cursor after loading; that
		// alone must not raise the bar the moment a reader nudges upward. Only pages that are
		// detached from the live edge (targeted or forward history) show it immediately.
		let detached_page = state.history_targeted || state.history_after.is_some();
		let unread = state
			.selected
			.is_some_and(|channel| state.missed(channel) == Some(true));
		// Messages that landed at the live edge after the reader last had it on screen.
		let new_below = if self.following || browsing_history || !self.at_current_latest {
			0
		} else {
			self.seen_latest
				.or(self.unread_dismissed.flatten())
				.map_or(0, |seen| {
					state.timeline.iter().filter(|m| m.id > seen).count()
				})
		};
		self.present_control = None;
		if (!self.following && distance_from_bottom > PRESENT_CONTROL_SCREENS * area.height())
			|| (self.target_browsing && !(at_bottom && unread))
			|| detached_page
			|| new_below > 0
		{
			// A control on the right edge, not a bar across the conversation: it says the
			// same thing with far less furniture and never covers a message being read.
			let label = (new_below > 0).then(|| {
				crate::i18n::translate_count(
					"timeline-present-control-new-messages",
					new_below,
					&[],
				)
			});
			let (rect, present) = present_control(ui, area, unread, label.as_deref());
			self.present_control = Some(rect);
			if present {
				if browsing_history {
					self.latest = true;
				}
				if distance_from_bottom > 0.5 && !browsing_history && !self.instant_scrolling {
					// Glide back so the reader keeps their place in the conversation.
					self.target_browsing = false;
					self.present_scroll = Some((output.state.clamped_offset().y, 0.0));
				} else {
					self.follow_latest(state);
				}
				ui.ctx().request_repaint();
			}
		}
		if let Some((message_id, attachment_id)) = self.pending_viewer
			&& state.timeline.get(message_id).is_some()
		{
			self.pending_viewer = None;
			self.embed_viewing = None;
			self.viewing = Some((message_id, attachment_id));
		}
		self.show_fullscreen_video(ui.ctx(), state);
		if let Some((message, media)) = self.download.embed_view_request.take()
			&& let Some(source) = display_message(state, message)
		{
			self.viewing = None;
			self.component_viewing = None;
			self.embed_viewing = Some((
				message,
				Revealed::fingerprint(source),
				model::Attachment {
					id: Id(0),
					filename: "Embed image.png".into(),
					description: None,
					content_type: Some("image/png".into()),
					size: 0,
					media,
					spoiler: false,
					duration_ms: None,
					waveform: Vec::new(),
				},
			));
		}
		if let Some((message, fingerprint, image)) = &self.embed_viewing {
			let allowed = !state.timeline.is_deleted(*message)
				&& display_message(state, *message).is_some_and(|source| {
					!source.embeds_suppressed
						&& Revealed::fingerprint(source) == *fingerprint
						&& (!crate::embeds::has_media_spoilers(source)
							|| self
								.revealed
								.get(message)
								.is_some_and(|reveal| reveal.media && reveal.matches(source)))
				});
			if !allowed
				|| crate::attachments::viewer(
					ui,
					std::slice::from_ref(image),
					image.id,
					crate::attachments::Floor::Fit,
					avatars,
					&mut self.download,
					&mut self.opening,
					state.demo,
				)
				.is_none()
			{
				self.embed_viewing = None;
			}
		}
		if let Some((message_id, attachment_id)) = self.viewing {
			let message = state
				.timeline
				.get(message_id)
				.or_else(|| {
					state
						.interactions
						.ephemeral
						.iter()
						.find(|message| message.id == message_id)
				})
				.filter(|m| {
					if let Some(identity) = self.component_viewing {
						identity
							== (
								m.id,
								egui::Id::unique((&m.components, &m.attachments)).value(),
							)
					} else {
						!crate::embeds::has_media_spoilers(m)
							|| self
								.revealed
								.get(&m.id)
								.is_some_and(|reveal| reveal.media && reveal.matches(m))
					}
				});
			self.viewing = message.and_then(|m| {
				crate::attachments::viewer(
					ui,
					if self.component_viewing.is_some() {
						m.attachments
							.iter()
							.find(|a| a.id == attachment_id)
							.map(std::slice::from_ref)
							.unwrap_or(&[])
					} else {
						&m.attachments
					},
					attachment_id,
					crate::attachments::Floor::Fit,
					avatars,
					&mut self.download,
					&mut self.opening,
					state.demo,
				)
				.map(|id| (message_id, id))
			});
		}
	}
}
