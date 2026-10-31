//! A thread: its subject, its labels, who is in it, and its messages stacked on the card that
//! fills the page; the earlier ones folded to a line, as Newton showed them, and a long run of
//! them tucked into one line with their count, as Gmail does. It opens on the first unread
//! message. On a wide window a sender clicked is shown beside it.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    App, Bounds, Context, Entity, MouseButton, PathPromptOptions, Pixels, ScrollHandle, SharedString, Window, canvas,
    div, point, px,
};
use mail_core::api::Command;

use super::body::BodyView;
use super::person::PersonView;
use crate::models::{Attachment, Conversation, MessageItem, PathReply, ReplyKind};
use crate::store::Store;
use crate::theme::*;
use crate::ui::{self, ActionButton, Chip, IconButton, MenuEntry, Symbol, Variant, icon};

pub struct ThreadPane {
    store: Entity<Store>,
    pub scroll: ScrollHandle,
    bodies: HashMap<String, Entity<BodyView>>,
    pane: Option<Entity<PersonView>>,
    shows_tucked: bool,
    /// Where each message is on the page, as last drawn, to scroll to.
    positions: Rc<RefCell<HashMap<String, Bounds<Pixels>>>>,
    shown: Option<String>,
    shown_focus: Option<String>,
    /// The scroll to make once the page is laid out.
    pending_scroll: Option<PendingScroll>,
    previews: HashMap<String, PathBuf>,
    asked_previews: HashSet<String>,
}

enum PendingScroll {
    Offset(f32),
    Message(String),
}

/// The largest picture fetched only to draw a card's preview.
const FETCHED_AHEAD: i64 = 5_000_000;

impl ThreadPane {
    pub fn new(store: Entity<Store>, _: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&store, |this, _, cx| this.store_changed(cx)).detach();
        Self {
            store,
            scroll: ScrollHandle::new(),
            bodies: HashMap::new(),
            pane: None,
            shows_tucked: false,
            positions: Rc::default(),
            shown: None,
            shown_focus: None,
            pending_scroll: None,
            previews: HashMap::new(),
            asked_previews: HashSet::new(),
        }
    }

    /// Takes the pages of the messages off the screen, while the thread isn't drawn.
    pub fn hide_pages(&self, cx: &mut App) {
        for body in self.bodies.values() {
            body.update(cx, |body, _| body.hide());
        }
    }

    fn store_changed(&mut self, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(open) = store.conversation.clone() else {
            self.shown = None;
            self.bodies.clear();
            self.pane = None;
            return cx.notify();
        };
        if self.shown.as_ref() != Some(&open.id) {
            self.shown = Some(open.id.clone());
            self.shows_tucked = false;
            self.bodies.clear();
            self.positions.borrow_mut().clear();
            self.pane = None;
            self.scroll.set_offset(point(px(0.), px(0.)));
            self.pending_scroll = match (store.thread_offset, store.focused_message.clone()) {
                (Some(offset), _) => Some(PendingScroll::Offset(offset as f32)),
                (None, Some(message)) => Some(PendingScroll::Message(message)),
                _ => None,
            };
            self.shown_focus = store.focused_message.clone();
        } else if store.focused_message != self.shown_focus {
            self.shown_focus = store.focused_message.clone();
            if let Some(message) = store.focused_message.clone() {
                self.pending_scroll = Some(PendingScroll::Message(message));
            }
        }
        self.sync_bodies(&open, cx);
        self.fetch_previews(&open, cx);
        cx.notify();
    }

    /// The bodies of the messages shown open; a folded one's goes, and its web view with it.
    fn sync_bodies(&mut self, open: &Conversation, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let shown: HashSet<String> = open
            .messages
            .iter()
            .filter(|message| message.html.is_some() && !store.is_folded(message))
            .map(|message| message.id.clone())
            .collect();
        self.bodies.retain(|id, _| shown.contains(id));
        for message in &open.messages {
            if !shown.contains(&message.id) {
                continue;
            }
            match self.bodies.get(&message.id) {
                Some(body) => body.update(cx, |body, cx| body.set_message(message.clone(), cx)),
                None => {
                    let store = self.store.clone();
                    let scroll = self.scroll.clone();
                    let message = message.clone();
                    let id = message.id.clone();
                    let body = cx.new(|cx| BodyView::new(store, message, scroll, cx));
                    cx.observe(&body, |_, _, cx| cx.notify()).detach();
                    self.bodies.insert(id, body);
                }
            }
        }
    }

    /// Fetches the images small enough to draw on their cards.
    fn fetch_previews(&mut self, open: &Conversation, cx: &mut Context<Self>) {
        for message in &open.messages {
            for (index, attachment) in message.attachments.iter().enumerate() {
                let key = format!("{}/{index}", message.id);
                if !attachment.mime.starts_with("image/")
                    || attachment.size > FETCHED_AHEAD
                    || !self.asked_previews.insert(key.clone())
                {
                    continue;
                }
                let answer = self
                    .store
                    .read(cx)
                    .core
                    .call::<PathReply>(Command::OpenAttachment { message: message.id.clone(), index });
                cx.spawn(async move |this, cx| {
                    let Ok(found) = answer.await else { return };
                    let _ = this.update(cx, |this, cx| {
                        this.previews.insert(key, PathBuf::from(found.path));
                        cx.notify();
                    });
                })
                .detach();
            }
        }
    }

    fn show_person(&mut self, email: String, wide: bool, cx: &mut Context<Self>) {
        if !wide {
            self.store.update(cx, |store, cx| {
                store.contact = Some(email);
                cx.notify();
            });
            return;
        }
        let store = self.store.clone();
        self.pane = Some(cx.new(|cx| PersonView::new(store, email, cx)));
        cx.notify();
    }

    fn open_attachment(&self, message: &str, index: usize, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.open_attachment(message, index, cx, |store, path, cx| {
                if let Err(error) = crate::platform::open_file(&path) {
                    store.show(error, cx);
                }
            })
        });
    }

    /// Copies files of a message where the user picks: one to a name, several to a folder.
    fn save_attachments(&self, message: &MessageItem, indices: Vec<usize>, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let core = store.read(cx).core.clone();
        let message_id = message.id.clone();
        let names: Vec<String> = indices.iter().map(|index| message.attachments[*index].name.clone()).collect();
        let single = indices.len() == 1;
        let downloads = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
        let picked = if single {
            Err(cx.prompt_for_new_path(&downloads, Some(&names[0])))
        } else {
            Ok(cx.prompt_for_paths(PathPromptOptions {
                files: false,
                directories: true,
                multiple: false,
                prompt: Some("Save".into()),
            }))
        };
        cx.spawn(async move |_, cx| {
            let targets: Vec<PathBuf> = match picked {
                Err(single) => match single.await {
                    Ok(Ok(Some(path))) => vec![path],
                    _ => return,
                },
                Ok(folder) => match folder.await {
                    Ok(Ok(Some(mut folders))) if !folders.is_empty() => {
                        let folder = folders.remove(0);
                        names.iter().map(|name| folder.join(name)).collect()
                    }
                    _ => return,
                },
            };
            for (index, target) in indices.into_iter().zip(targets) {
                let found =
                    core.call::<PathReply>(Command::OpenAttachment { message: message_id.clone(), index }).await;
                let copied = match found {
                    Ok(found) => {
                        cx.background_spawn(async move {
                            std::fs::copy(&found.path, &target).map(|_| ()).map_err(|error| error.to_string())
                        })
                        .await
                    }
                    Err(error) => Err(error),
                };
                if let Err(error) = copied {
                    store.update(cx, |store, cx| store.show(error, cx));
                    return;
                }
            }
        })
        .detach();
    }

    /// Scrolls where the thread was left, or to its message, once the page is laid out.
    fn scroll_when_laid_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pending) = self.pending_scroll.take() else { return };
        match pending {
            PendingScroll::Offset(offset) => self.scroll.set_offset(point(px(0.), px(-offset))),
            PendingScroll::Message(message) => {
                let first = self
                    .store
                    .read(cx)
                    .conversation
                    .as_ref()
                    .and_then(|open| open.messages.first())
                    .map(|first| first.id.clone());
                if first.as_ref() == Some(&message) {
                    self.scroll.set_offset(point(px(0.), px(0.)));
                    return;
                }
                let Some(bounds) = self.positions.borrow().get(&message).copied() else {
                    self.pending_scroll = Some(PendingScroll::Message(message));
                    window.request_animation_frame();
                    return;
                };
                let viewport = self.scroll.bounds();
                let offset = self.scroll.offset();
                let top = bounds.origin.y - viewport.origin.y - offset.y;
                let max = self.scroll.max_offset().y;
                let wanted = (top - px(space::L)).max(px(0.)).min(max);
                self.scroll.set_offset(point(px(0.), -wanted));
            }
        }
        cx.notify();
    }

    fn header(&self, open: &Conversation, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let store = self.store.clone();
        let thread = open.id.clone();
        let participants = div()
            .text_in(TextStyle::Label, MUTED_FOREGROUND, cx)
            .child(open.from.clone())
            .when(!open.to.is_empty(), |line| {
                line.child(div().text_color(cx.color(MUTED_STRONGER_FOREGROUND)).child(" to ")).child(open.to.clone())
            })
            .flex()
            .flex_wrap();
        let chips: Vec<_> = open
            .labels
            .iter()
            .map(|label| {
                Chip::new(SharedString::from(format!("label-{}", label.id)), label.name.clone())
                    .symbol(Symbol::Tag)
                    .on_card()
                    .into_any_element()
            })
            .chain(open.muted.then(|| {
                Chip::new("muted", "Muted")
                    .symbol(Symbol::BellOff)
                    .on_card()
                    .help("New mail in this thread stays out of the inbox")
                    .into_any_element()
            }))
            .collect();
        let unsubscribe = open.unsubscribe.then(|| {
            let store = store.clone();
            let thread = thread.clone();
            ActionButton::new("unsubscribe", "Unsubscribe").symbol(Symbol::MailX).variant(Variant::Ghost).on_click(
                move |_, _, cx| {
                    store.update(cx, |store, cx| {
                        store.act(crate::models::Action::Unsubscribe, std::slice::from_ref(&thread), None, None, cx)
                    })
                },
            )
        });
        let blocked = open.messages.iter().any(|message| message.blocked_images);
        let star = store.clone();
        let star_thread = thread.clone();
        let images = store.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(space::S + 2.))
            .pb(px(space::XL))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(space::M))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(space::XS))
                            .child(
                                div()
                                    .text(TextStyle::Title, cx)
                                    .child(ui::RichText::new("thread-subject", open.subject.clone())),
                            )
                            .child(participants),
                    )
                    .child(ui::StarButton::new("thread-star", open.starred, move |_, _, cx| {
                        star.update(cx, |store, cx| store.toggle_star(std::slice::from_ref(&star_thread), cx))
                    })),
            )
            .when(!chips.is_empty() || unsubscribe.is_some(), |header| {
                header.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::M))
                        .child(div().flex_1().flex().flex_wrap().gap(px(space::XS)).children(chips))
                        .children(unsubscribe),
                )
            })
            .when(blocked, |header| {
                header.child(ui::notice(
                    "Images from the sender were left out.",
                    ui::Tone::Muted,
                    None,
                    Some(
                        ActionButton::new("load-images", "Load images")
                            .on_click(move |_, _, cx| images.update(cx, |store, cx| store.show_images(cx))),
                    ),
                    cx,
                ))
            })
    }

    fn message_card(
        &self,
        message: &MessageItem,
        folded: bool,
        wide: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let store = self.store.clone();
        let this = cx.entity();
        let state = store.read(cx);
        let thread = state.conversation.as_ref().map(|open| open.id.clone()).unwrap_or_default();
        let original = state.original_messages.contains(&message.id);
        let can_original = !folded && message.designed && message.html.is_some() && cx.dark() && state.dark_mail();
        let toggle = {
            let store = store.clone();
            let message = message.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                store.update(cx, |store, cx| store.toggle_fold(&message, cx))
            }
        };
        let tap_sender = {
            let this = this.clone();
            let toggle = toggle.clone();
            let email = message.from_email.clone();
            move |event: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                cx.stop_propagation();
                if folded {
                    return toggle(event, window, cx);
                }
                this.update(cx, |this, cx| this.show_person(email.clone(), wide, cx));
            }
        };
        let reply = |kind: ReplyKind| {
            let store = store.clone();
            let thread = thread.clone();
            let message = message.id.clone();
            move |_: &mut Window, cx: &mut App| {
                store.update(cx, |store, cx| store.reply(kind, Some(thread.clone()), Some(message.clone()), cx))
            }
        };
        let actions = (!folded).then(|| {
            let reply_now = reply(ReplyKind::Reply);
            let reply_all = reply(ReplyKind::ReplyAll);
            let forward = reply(ReplyKind::Forward);
            let original_store = store.clone();
            let original_id = message.id.clone();
            div()
                .flex()
                .items_center()
                .mr(px(-size::BUTTON_GAP / 2.))
                .when(can_original, |row| {
                    row.child(
                        IconButton::new(
                            SharedString::from(format!("original-{}", message.id)),
                            if original { Symbol::Moon } else { Symbol::Sun },
                            if original { "Show in dark colours" } else { "Show the original" },
                        )
                        .size(ControlSize::Small)
                        .plain()
                        .quiet()
                        .on_click(move |_, _, cx| {
                            original_store.update(cx, |store, cx| store.toggle_original(&original_id, cx))
                        }),
                    )
                })
                .child(
                    IconButton::new(SharedString::from(format!("reply-{}", message.id)), Symbol::Reply, "Reply")
                        .size(ControlSize::Small)
                        .plain()
                        .on_click(move |_, window, cx| reply_now(window, cx)),
                )
                .child(
                    IconButton::new(SharedString::from(format!("more-{}", message.id)), Symbol::Ellipsis, "More")
                        .size(ControlSize::Small)
                        .plain()
                        .menu(move |_, _| {
                            let reply_all = reply_all.clone();
                            let forward = forward.clone();
                            vec![
                                MenuEntry::item("Reply all", move |window, cx| reply_all(window, cx))
                                    .symbol(Symbol::ReplyAll),
                                MenuEntry::item("Forward", move |window, cx| forward(window, cx))
                                    .symbol(Symbol::Forward),
                            ]
                        }),
                )
        });
        let name_style = if folded { TextStyle::Body } else { TextStyle::Subheading };
        let name_color = if folded { MUTED_STRONGER_FOREGROUND } else { FOREGROUND };
        let under = if folded { message.snippet.clone() } else { message.to.clone() };
        let head = div()
            .id(SharedString::from(format!("head-{}", message.id)))
            .flex()
            .items_center()
            .gap(px(space::M))
            .cursor_pointer()
            .on_click(toggle.clone())
            .child(
                div()
                    .id(SharedString::from(format!("avatar-{}", message.id)))
                    .on_click(tap_sender.clone())
                    .child(ui::Avatar::new(&message.initials, &message.from_email, size::MESSAGE_AVATAR)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .id(SharedString::from(format!("name-{}", message.id)))
                            .on_click(tap_sender)
                            .when(!folded, |name| {
                                let email = message.from_email.clone();
                                name.tooltip(move |_, cx| ui::Tooltip::view(email.clone(), cx))
                            })
                            .child(ui::line_in(message.from_name.clone(), name_style, name_color, cx)),
                    )
                    .child(ui::line(under, TextStyle::Caption, cx)),
            )
            .when(!message.attachments.is_empty(), |head| {
                head.child(icon(Symbol::Paperclip, 13.).text_color(cx.color(MUTED_STRONGER_FOREGROUND)))
            })
            .child(div().text(TextStyle::Caption, cx).whitespace_nowrap().child(message.date.clone()))
            .children(actions);
        let body = (!folded).then(|| {
            let content = if let Some(body) = self.bodies.get(&message.id) {
                body.clone().into_any_element()
            } else if message.failed {
                ui::paragraph_in(
                    "This message couldn't be loaded. Open the conversation again to retry.",
                    TextStyle::Body,
                    MUTED_STRONGER_FOREGROUND,
                    cx,
                )
                .into_any_element()
            } else {
                ui::body_placeholder(message.shape.clone(), cx).into_any_element()
            };
            div().pl(px(size::BODY_INDENT)).child(content)
        });
        let files = (!folded && !message.attachments.is_empty()).then(|| self.attachments(message, cx));
        let positions = self.positions.clone();
        let id = message.id.clone();
        div()
            .id(SharedString::from(format!("message-{}", message.id)))
            .relative()
            .flex()
            .flex_col()
            .gap(px(space::M))
            .py(px(space::L))
            .when(folded, |card| card.cursor_pointer().on_click(toggle))
            .child(
                canvas(
                    move |bounds, _, _| {
                        positions.borrow_mut().insert(id.clone(), bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .child(head)
            .children(body)
            .children(files.map(|files| div().pl(px(size::BODY_INDENT)).child(files)))
    }

    /// The files under a message: their count, Save all, and a card each.
    fn attachments(&self, message: &MessageItem, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let count = message.attachments.len();
        let this = cx.entity();
        let downloading = self.store.read(cx).downloading.clone();
        let cards: Vec<_> = message
            .attachments
            .iter()
            .enumerate()
            .map(|(index, attachment): (usize, &Attachment)| {
                let key = format!("{}/{index}", message.id);
                let open = this.clone();
                let menu = this.clone();
                let message_id = message.id.clone();
                let menu_message = message.clone();
                let card = ui::attachment_card(
                    SharedString::from(format!("file-{key}")),
                    &attachment.name,
                    &attachment.mime,
                    attachment.size,
                    self.previews.get(&key).cloned(),
                    downloading.contains(&key),
                    Rc::new(move |_, _, cx| open.update(cx, |this, cx| this.open_attachment(&message_id, index, cx))),
                    cx,
                );
                let menu_id = SharedString::from(format!("file-menu-{key}"));
                div()
                    .id(SharedString::from(format!("file-wrap-{key}")))
                    .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                        let open = menu.clone();
                        let save = menu.clone();
                        let message = menu_message.clone();
                        let open_message = message.id.clone();
                        let entries = vec![
                            MenuEntry::item("Open", move |_, cx| {
                                open.update(cx, |this, cx| this.open_attachment(&open_message, index, cx))
                            }),
                            MenuEntry::item("Save", move |_, cx| {
                                save.update(cx, |this, cx| this.save_attachments(&message, vec![index], cx))
                            }),
                        ];
                        ui::open_menu(menu_id.clone(), entries, Some(event.position), window, cx);
                    })
                    .child(card)
            })
            .collect();
        let save_all = (count > 1).then(|| {
            let this = this.clone();
            let message = message.clone();
            ActionButton::new(SharedString::from(format!("save-all-{}", message.id)), "Save all")
                .symbol(Symbol::Download)
                .variant(Variant::Ghost)
                .size(ControlSize::Small)
                .on_click(move |_, _, cx| {
                    this.update(cx, |this, cx| this.save_attachments(&message, (0..count).collect(), cx))
                })
        });
        div()
            .pt(px(space::XS))
            .flex()
            .flex_col()
            .gap(px(space::S))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(div().flex_1().text(TextStyle::Caption, cx).child(if count == 1 {
                        "1 attachment".to_string()
                    } else {
                        format!("{count} attachments")
                    }))
                    .children(save_all.map(|button| div().mr(px(-size::BUTTON_GAP / 2.)).child(button))),
            )
            .child(div().flex().flex_wrap().gap(px(space::S)).children(cards))
    }

    /// The messages tucked into one line: of a run of at least five folded ones, all but its first
    /// and its last two.
    fn tucked(&self, open: &Conversation, store: &Store) -> (HashSet<String>, HashMap<String, usize>) {
        let mut ids = HashSet::new();
        let mut starts = HashMap::new();
        if self.shows_tucked {
            return (ids, starts);
        }
        let mut run: Vec<&MessageItem> = Vec::new();
        for message in open.messages.iter().map(Some).chain([None]) {
            if let Some(message) = message
                && store.is_folded(message)
            {
                run.push(message);
                continue;
            }
            if run.len() >= 5 {
                let middle = &run[1..run.len() - 2];
                ids.extend(middle.iter().map(|message| message.id.clone()));
                starts.insert(middle[0].id.clone(), middle.len());
            }
            run.clear();
        }
        (ids, starts)
    }
}

impl Render for ThreadPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(open) = self.store.read(cx).conversation.clone() else { return div().size_full().into_any_element() };
        if self.shown.as_ref() != Some(&open.id) {
            self.store_changed(cx);
        }
        self.scroll_when_laid_out(window, cx);
        // Only the open thread's own scroll, once it is where it opens: back at the top too.
        let offset: f64 = (-f32::from(self.scroll.offset().y)).into();
        let kept = self.store.read(cx).thread_offset;
        let moved = match kept {
            Some(kept) => (kept - offset).abs() > 1.,
            None => offset > 1.,
        };
        if moved && self.pending_scroll.is_none() && self.shown.as_ref() == Some(&open.id) {
            self.store.update(cx, |store, _| store.note_thread_scroll(offset));
        }
        let width: f32 = window.bounds().size.width.into();
        let wide = width >= size::PANE_FROM;
        let store = self.store.read(cx);
        let (tucked, starts) = self.tucked(&open, store);
        let last = open.messages.last().map(|message| message.id.clone());
        let mut children: Vec<gpui::AnyElement> = vec![self.header(&open, cx).into_any_element()];
        for message in &open.messages {
            if let Some(count) = starts.get(&message.id) {
                let this = cx.entity();
                children.push(
                    ui::folded_count(
                        SharedString::from(format!("tucked-{}", message.id)),
                        *count,
                        Rc::new(move |_, _, cx| {
                            this.update(cx, |this, cx| {
                                this.shows_tucked = true;
                                cx.notify();
                            })
                        }),
                        cx,
                    )
                    .into_any_element(),
                );
                children.push(ui::rule(CARD_RULE, cx).into_any_element());
                continue;
            }
            if tucked.contains(&message.id) {
                continue;
            }
            let folded = self.store.read(cx).is_folded(message);
            children.push(self.message_card(message, folded, wide, cx).into_any_element());
            if Some(&message.id) != last.as_ref() {
                children.push(ui::rule(CARD_RULE, cx).into_any_element());
            }
        }
        if let Some(draft) = open.draft_id.clone() {
            let store = self.store.clone();
            let again = self.store.clone();
            let again_draft = draft.clone();
            children.push(
                div()
                    .id("draft-card")
                    .flex()
                    .flex_col()
                    .cursor_pointer()
                    .on_click(move |_, _, cx| store.update(cx, |store, cx| store.open_draft(&draft, cx)))
                    .child(ui::rule(CARD_RULE, cx))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::M))
                            .py(px(space::L))
                            .child(icon(Symbol::Pencil, 14.).text_color(cx.color(DESTRUCTIVE)))
                            .child(div().text_in(TextStyle::Subheading, DESTRUCTIVE, cx).child("Draft"))
                            .child(
                                div()
                                    .flex_1()
                                    .text_in(TextStyle::Label, MUTED_STRONGER_FOREGROUND, cx)
                                    .child("A reply you started"),
                            )
                            .child(ActionButton::new("open-draft", "Open").on_click(move |_, _, cx| {
                                again.update(cx, |store, cx| store.open_draft(&again_draft, cx))
                            })),
                    )
                    .into_any_element(),
            );
        }
        let reply = |id: &'static str, symbol: Symbol, help: &'static str, kind: ReplyKind| {
            let store = self.store.clone();
            let thread = open.id.clone();
            IconButton::new(id, symbol, help).on_card().on_click(move |_, _, cx| {
                store.update(cx, |store, cx| store.reply(kind, Some(thread.clone()), None, cx))
            })
        };
        children.push(
            div()
                .flex()
                .items_center()
                .ml(px(-size::BUTTON_GAP / 2.))
                .pt(px(space::XL + 2.))
                .child(reply("reply-all", Symbol::ReplyAll, "Reply all", ReplyKind::ReplyAll))
                .child(reply("reply", Symbol::Reply, "Reply", ReplyKind::Reply))
                .child(reply("forward", Symbol::Forward, "Forward", ReplyKind::Forward))
                .into_any_element(),
        );
        // A press outside the card closes the thread; one inside goes on to select its text.
        let inside = Rc::new(Cell::new(false));
        let pressed = inside.clone();
        let close = self.store.clone();
        let page = div()
            .id("thread-scroll")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                if !inside.replace(false) {
                    close.update(cx, |store, cx| store.close(cx))
                }
            })
            .child(
                div().w_full().flex().justify_center().px(px(size::CARD_MARGIN_X)).py(px(size::CARD_MARGIN_Y)).child(
                    ui::page_card(cx)
                        .id("thread-card")
                        .w_full()
                        .max_w(px(size::CARD_WIDTH))
                        .px(px(size::CARD_INSET))
                        .py(px(space::XXL))
                        .on_mouse_down(MouseButton::Left, move |_, _, _| pressed.set(true))
                        .flex()
                        .flex_col()
                        .children(children),
                ),
            );
        let pane = self.pane.clone().filter(|_| wide).map(|pane| {
            let this = cx.entity();
            div()
                .relative()
                .w(px(size::PANE_WIDTH))
                .h_full()
                .flex_none()
                .border_l_1()
                .border_color(cx.color(BORDER))
                .child(pane)
                .child(div().absolute().top(px(space::M)).right(px(space::M)).child(
                    IconButton::new("pane-close", Symbol::X, "Close").plain().on_click(move |_, _, cx| {
                        this.update(cx, |this, cx| {
                            this.pane = None;
                            cx.notify();
                        })
                    }),
                ))
        });
        div().size_full().flex().bg(cx.color(BACKGROUND)).child(page).children(pane).into_any_element()
    }
}
