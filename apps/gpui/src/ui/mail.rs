//! The pieces of mail: a sender's avatar, chips, a message's files, the folded count, the
//! placeholder of a page on its way, the app's and Google's marks, key caps, the account
//! colours and the switch.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{App, ClickEvent, Global, ObjectFit, SharedString, Window, canvas, div, img, px, svg};
use mail_core::api::Command;

use super::{OnClick, PlainButton, Symbol, Tooltip, icon, line, line_in, spinner};
use crate::core::Core;
use crate::models::{AvatarReply, ShapeLine};
use crate::theme::*;

/// Senders' pictures from the server, as files on this device; none for who has none.
#[derive(Default)]
pub struct Photos {
    pub core: Option<Core>,
    paths: HashMap<String, Option<PathBuf>>,
    asked: HashSet<String>,
}

impl Global for Photos {}

impl Photos {
    pub fn new(core: Core) -> Self {
        Self { core: Some(core), ..Default::default() }
    }

    /// The sender's picture, asked for the first time it is wanted.
    pub fn path(email: &str, cx: &mut App) -> Option<PathBuf> {
        let key = email.to_lowercase();
        let photos = cx.try_global::<Photos>()?;
        if let Some(path) = photos.paths.get(&key) {
            return path.clone();
        }
        if photos.asked.contains(&key) {
            return None;
        }
        let core = photos.core.clone()?;
        cx.global_mut::<Photos>().asked.insert(key.clone());
        let answer = core.call::<AvatarReply>(Command::Avatar { email: key.clone() });
        cx.spawn(async move |cx| {
            let path = answer.await.ok().and_then(|reply| reply.path).map(PathBuf::from);
            cx.update(|cx| {
                cx.global_mut::<Photos>().paths.insert(key, path.clone());
                if path.is_some() {
                    cx.refresh_windows();
                }
            });
        })
        .detach();
        None
    }
}

/// Someone's picture, else a circle with their initials in a colour picked from their address.
#[derive(IntoElement)]
pub struct Avatar {
    initials: SharedString,
    email: SharedString,
    side: f32,
}

impl Avatar {
    pub fn new(initials: &str, email: &str, side: f32) -> Self {
        Self { initials: initials.to_string().into(), email: email.to_string().into(), side }
    }
}

impl RenderOnce for Avatar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let circle = div().size(px(self.side)).flex_none().rounded_full().overflow_hidden();
        if let Some(path) = Photos::path(&self.email, cx) {
            return circle.child(img(path).size_full().rounded_full().object_fit(ObjectFit::Cover));
        }
        circle
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.color(sender_color(&self.email)))
            .text_color(cx.color(PRIMARY_FOREGROUND))
            .font_family(FONT)
            .text_size(px((self.side * 0.4).round()))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .child(self.initials)
    }
}

/// A small pill: an address, a file, a thread's label. `invalid` can't be sent to.
#[derive(IntoElement)]
pub struct Chip {
    id: SharedString,
    title: SharedString,
    symbol: Option<Symbol>,
    detail: Option<SharedString>,
    invalid: bool,
    pending: bool,
    on_card: bool,
    help: Option<SharedString>,
    remove: Option<OnClick>,
    action: Option<OnClick>,
}

impl Chip {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            symbol: None,
            detail: None,
            invalid: false,
            pending: false,
            on_card: false,
            help: None,
            remove: None,
            action: None,
        }
    }

    pub fn symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn on_card(mut self) -> Self {
        self.on_card = true;
        self
    }

    pub fn help(mut self, help: impl Into<SharedString>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn remove(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.remove = Some(Rc::new(handler));
        self
    }

    pub fn action(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.action = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (fill, hover) = if self.on_card {
            (CARD_ACCENT, CARD_ACCENT_STRONGER)
        } else {
            (BACKGROUND_SECONDARY, BACKGROUND_SECONDARY_ACCENT)
        };
        let color = if self.invalid { DESTRUCTIVE } else { FOREGROUND };
        let glyph_color = if self.invalid { DESTRUCTIVE } else { MUTED_FOREGROUND };
        let help = self.help.clone().unwrap_or_else(|| self.title.clone());
        let clickable = self.action.is_some() && self.remove.is_none();
        let action = self.action.clone();
        let remove = self.remove.clone().map(|remove| {
            let remove_fill = if self.on_card { CARD_ACCENT_STRONGER } else { BACKGROUND_SECONDARY_ACCENT_STRONGER };
            let title = self.title.clone();
            div()
                .id(SharedString::from(format!("{}-remove", self.id)))
                .size(px(size::CHIP))
                .mr(px(-(space::S + 2.)))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(cx.color(MUTED_STRONGER_FOREGROUND))
                .child(
                    div()
                        .size(px(size::CHIP - space::XXS * 2.))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .hover(|circle| circle.bg(cx.color(remove_fill)).text_color(cx.color(FOREGROUND)))
                        .child(icon(Symbol::X, 11.)),
                )
                .tooltip(move |_, cx| Tooltip::view(format!("Remove {title}"), cx))
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    remove(event, window, cx);
                })
        });
        let glyph = if self.pending {
            Some(spinner(12., cx.color(glyph_color)).into_any_element())
        } else {
            self.symbol.map(|symbol| icon(symbol, 12.).text_color(cx.color(glyph_color)).into_any_element())
        };
        div()
            .id(self.id.clone())
            .flex()
            .flex_none()
            .items_center()
            .gap(px(space::XS))
            .h(px(size::CHIP))
            .px(px(space::S + 2.))
            .max_w(px(320.))
            .rounded_full()
            .bg(cx.color(fill))
            .text_in(TextStyle::Caption, color, cx)
            .when(self.invalid, |chip| chip.border_1().border_color(cx.tint(DESTRUCTIVE, opacity::COLOR_BORDER)))
            .when(clickable, |chip| chip.cursor_pointer().hover(|chip| chip.bg(cx.color(hover))))
            .tooltip(move |_, cx| Tooltip::view(help.clone(), cx))
            .when_some(action.filter(|_| clickable), |chip, action| {
                chip.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    action(event, window, cx);
                })
            })
            .children(glyph)
            .child(div().truncate().min_w_0().child(self.title.clone()))
            .when_some(self.detail.clone(), |chip, detail| {
                chip.child(line(detail, TextStyle::Footnote, cx).flex_none())
            })
            .children(remove)
    }
}

/// The icon of a file's kind, by its type.
pub fn file_kind(mime: &str) -> Symbol {
    let mime = mime.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|word| mime.contains(word));
    if mime.starts_with("image/") {
        Symbol::FileImage
    } else if mime.starts_with("video/") {
        Symbol::FileVideo
    } else if mime.starts_with("audio/") {
        Symbol::FileAudio
    } else if has(&["spreadsheet", "excel", "csv", "numbers"]) {
        Symbol::FileSpreadsheet
    } else if has(&["presentation", "powerpoint", "keynote"]) {
        Symbol::Presentation
    } else if has(&["zip", "compressed", "tar", "rar", "7z", "gzip"]) {
        Symbol::FileArchive
    } else if has(&["pdf", "text", "word", "document", "rtf", "pages"]) {
        Symbol::FileText
    } else {
        Symbol::File
    }
}

/// A message's file as a card, as Gmail shows them: its picture (or its kind's icon) over its
/// name, kind and size.
pub fn attachment_card(
    id: SharedString,
    name: &str,
    mime: &str,
    bytes: i64,
    preview: Option<PathBuf>,
    pending: bool,
    on_click: OnClick,
    cx: &App,
) -> impl IntoElement + use<> {
    let kind = file_kind(mime);
    let extension =
        std::path::Path::new(name).extension().map(|ext| ext.to_string_lossy().to_uppercase()).unwrap_or_default();
    let detail = if extension.is_empty() || extension.len() > 5 {
        super::file_size(bytes)
    } else {
        format!("{extension} · {}", super::file_size(bytes))
    };
    let group = SharedString::from(format!("{id}-card"));
    let picture = match preview {
        Some(path) => img(path).size_full().object_fit(ObjectFit::Cover).into_any_element(),
        None => icon(kind, 28.).text_color(cx.color(MUTED_STRONGER_FOREGROUND)).into_any_element(),
    };
    let glyph = if pending {
        spinner(14., cx.color(MUTED_FOREGROUND)).into_any_element()
    } else {
        icon(kind, 14.).text_color(cx.color(MUTED_FOREGROUND)).into_any_element()
    };
    let help = format!("Open {name}");
    div()
        .id(id)
        .group(group.clone())
        .w(px(size::ATTACHMENT_WIDTH))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(radius::MD))
        .border_1()
        .border_color(cx.color(CARD_RULE))
        .cursor_pointer()
        .tooltip(move |_, cx| Tooltip::view(help.clone(), cx))
        .on_click(move |event, window, cx| on_click(event, window, cx))
        .child(
            div()
                .h(px(size::ATTACHMENT_PREVIEW))
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .bg(cx.color(CARD_ACCENT))
                .group_hover(group.clone(), |area| area.bg(cx.color(CARD_ACCENT_STRONGER)))
                .child(picture),
        )
        .child(div().h(px(size::HAIRLINE)).bg(cx.color(CARD_RULE)))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(space::XS))
                .px(px(space::S))
                .py(px(space::XS))
                .bg(cx.color(CARD_FILL))
                .group_hover(group, |footer| footer.bg(cx.color(CARD_ACCENT)))
                .child(glyph)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(1.))
                        .child(line_in(name.to_string(), TextStyle::Caption, FOREGROUND, cx))
                        .child(line(detail, TextStyle::Footnote, cx)),
                ),
        )
}

/// A long thread's run of folded messages tucked into one line, their count in a circle.
pub fn folded_count(id: SharedString, count: usize, on_click: OnClick, cx: &App) -> impl IntoElement + use<> {
    let group = SharedString::from(format!("{id}-group"));
    div()
        .id(id)
        .group(group.clone())
        .flex()
        .items_center()
        .gap(px(space::M))
        .py(px(space::M))
        .cursor_pointer()
        .tooltip(move |_, cx| Tooltip::view(format!("Show {count} earlier messages"), cx))
        .on_click(move |event, window, cx| on_click(event, window, cx))
        .child(
            div()
                .size(px(size::MESSAGE_AVATAR))
                .rounded_full()
                .border_1()
                .border_color(cx.color(CARD_RULE))
                .flex()
                .items_center()
                .justify_center()
                .text_in(TextStyle::LabelStrong, MUTED_FOREGROUND, cx)
                .group_hover(group.clone(), |circle| circle.bg(cx.color(CARD_ACCENT)))
                .child(count.to_string()),
        )
        .child(
            div()
                .text_in(TextStyle::Body, MUTED_STRONGER_FOREGROUND, cx)
                .group_hover(group, |words| words.text_color(cx.color(FOREGROUND)))
                .child(format!("{count} earlier messages")),
        )
}

/// Where a message's page will be while it is on its way: a bar for every line of its text and
/// a block for every image, so the thread is about as tall as it will be.
pub fn body_placeholder(shape: Vec<ShapeLine>, cx: &App) -> impl IntoElement + use<> {
    let fill = cx.color(CARD_ACCENT);
    let height = placeholder_height(&shape, 600.);
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let width: f32 = bounds.size.width.into();
            for block in placeholder_blocks(&shape, width) {
                let origin = bounds.origin + gpui::point(px(block.0), px(block.1));
                let quad = gpui::fill(gpui::Bounds::new(origin, gpui::size(px(block.2), px(block.3))), fill)
                    .corner_radii(px(radius::XS));
                window.paint_quad(quad);
            }
        },
    )
    .w_full()
    .h(px(height))
}

const LINE_HEIGHT: f32 = 23.25;
const BAR_HEIGHT: f32 = 10.;
const CHARACTER_WIDTH: f32 = 7.2;
const TALLEST: f32 = 1600.;
const REACH: [f32; 5] = [1., 0.94, 0.97, 0.9, 0.98];

/// The bars and blocks at this width, as (x, y, width, height).
fn placeholder_blocks(shape: &[ShapeLine], width: f32) -> Vec<(f32, f32, f32, f32)> {
    let mut blocks = Vec::new();
    let mut height = 0.;
    if width <= 0. {
        return blocks;
    }
    let per_line = (width / CHARACTER_WIDTH).max(1.) as u32;
    for line in shape {
        if height >= TALLEST {
            break;
        }
        match *line {
            ShapeLine::Text(0) => height += LINE_HEIGHT,
            ShapeLine::Text(length) => {
                let full = (length - 1) / per_line;
                for _ in 0..full {
                    let reach = REACH[blocks.len() % REACH.len()];
                    blocks.push((0., height + (LINE_HEIGHT - BAR_HEIGHT) / 2., width * reach, BAR_HEIGHT));
                    height += LINE_HEIGHT;
                }
                let rest = ((length - full * per_line) as f32 / per_line as f32).max(0.06);
                blocks.push((0., height + (LINE_HEIGHT - BAR_HEIGHT) / 2., width * rest, BAR_HEIGHT));
                height += LINE_HEIGHT;
            }
            ShapeLine::Image { width: image_width, height: image_height } => {
                let shown = (image_width.max(1) as f32).min(width);
                let tall = image_height as f32 * shown / image_width.max(1) as f32;
                blocks.push((0., height, shown, tall));
                height += tall;
            }
        }
    }
    blocks
}

/// How tall the placeholder stands at this width: at least a line.
pub fn placeholder_height(shape: &[ShapeLine], width: f32) -> f32 {
    let mut height: f32 = 0.;
    let per_line = (width / CHARACTER_WIDTH).max(1.) as u32;
    for line in shape {
        if height >= TALLEST {
            break;
        }
        height += match *line {
            ShapeLine::Text(0) => LINE_HEIGHT,
            ShapeLine::Text(length) => LINE_HEIGHT * ((length - 1) / per_line + 1) as f32,
            ShapeLine::Image { width: image_width, height: image_height } => {
                image_height as f32 * (image_width.max(1) as f32).min(width) / image_width.max(1) as f32
            }
        };
    }
    height.max(LINE_HEIGHT)
}

/// The app's icon drawn small: its W on a tile.
pub fn app_mark(side: f32, cx: &App) -> impl IntoElement + use<> {
    div()
        .size(px(side))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(side * 0.225))
        .bg(cx.color(CARD))
        .border_1()
        .border_color(cx.color(BORDER))
        .shadow(cx.shadow(ShadowSize::Sm, opacity::SHADOW))
        .child(svg().path("wonnet/app-mark.svg").size(px(side * 0.46)).text_color(cx.color(EMPHASIZED_FOREGROUND)))
}

/// Google's G, in its four colours.
pub fn google_mark(side: f32, cx: &App) -> impl IntoElement + use<> {
    let parts = [
        ("wonnet/google-blue.svg", GOOGLE_BLUE),
        ("wonnet/google-green.svg", GOOGLE_GREEN),
        ("wonnet/google-yellow.svg", GOOGLE_YELLOW),
        ("wonnet/google-red.svg", GOOGLE_RED),
    ];
    div().relative().size(px(side)).children(
        parts.map(|(path, color)| {
            svg().path(path).absolute().top_0().left_0().size(px(side)).text_color(cx.color(color))
        }),
    )
}

/// Microsoft's four squares.
pub fn microsoft_mark(side: f32, cx: &App) -> impl IntoElement + use<> {
    let parts = [
        ("wonnet/microsoft-red.svg", MICROSOFT_RED),
        ("wonnet/microsoft-green.svg", MICROSOFT_GREEN),
        ("wonnet/microsoft-blue.svg", MICROSOFT_BLUE),
        ("wonnet/microsoft-yellow.svg", MICROSOFT_YELLOW),
    ];
    div().relative().size(px(side)).children(
        parts.map(|(path, color)| {
            svg().path(path).absolute().top_0().left_0().size(px(side)).text_color(cx.color(color))
        }),
    )
}

/// Keys to press, one cap each.
pub fn key_caps(caps: Vec<String>, cx: &App) -> impl IntoElement + use<> {
    div().flex().items_center().gap(px(space::XXS)).children(caps.into_iter().map(|cap| {
        div()
            .min_w(px(22.))
            .h(px(20.))
            .px(px(space::XS))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::XS))
            .bg(cx.color(BACKGROUND_SECONDARY))
            .border_1()
            .border_color(cx.color(BORDER))
            .text_in(TextStyle::FootnoteStrong, FOREGROUND, cx)
            .child(cap)
    }))
}

/// The account colours on a line, to pick one: the chosen one wears a ring.
pub fn swatch_row(
    id: &str,
    selected: &str,
    pick: Rc<dyn Fn(String, &mut Window, &mut App)>,
    cx: &App,
) -> impl IntoElement + use<> {
    div().flex().items_center().children(account_color_names().into_iter().enumerate().map(|(index, name)| {
        let color = account_color(&name);
        let chosen = name == selected;
        let pick = pick.clone();
        let picked = name.clone();
        PlainButton::new(
            SharedString::from(format!("{id}-{name}")),
            div()
                .size(px(18.))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .border(px(1.5))
                .border_color(if chosen { cx.color(FOREGROUND) } else { gpui::transparent_black() })
                .child(div().size(px(12.)).rounded_full().bg(cx.color(color))),
        )
        .tint(color)
        .help(format!("Colour {}", index + 1))
        .on_click(move |_, window, cx| pick(picked.clone(), window, cx))
    }))
}

/// A switch, in the primary colour when on.
pub fn switch(
    id: impl Into<SharedString>,
    on: bool,
    toggle: impl Fn(bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let (width, height) = (32., 18.);
    div()
        .id(id.into())
        .w(px(width))
        .h(px(height))
        .flex_none()
        .rounded_full()
        .p(px(2.))
        .flex()
        .when(on, |track| track.justify_end())
        .bg(if on { cx.color(PRIMARY) } else { cx.color(BACKGROUND_ACCENT_STRONGEST) })
        .cursor_pointer()
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            toggle(!on, window, cx);
        })
        .child(
            div()
                .size(px(height - 4.))
                .rounded_full()
                .bg(cx.color(CARD))
                .shadow(cx.shadow(ShadowSize::Sm, opacity::SHADOW)),
        )
}
