//! What the app is drawn with: the colours, opacities, radii and shadows of
//! `packages/theme/tokens.json` (in `tokens.rs`, generated), the text sizes and the distances.
//! Views take every colour, size and distance from here, never a number of their own.

#[rustfmt::skip]
#[allow(dead_code)]
mod tokens;

use gpui::{App, BoxShadow, FontWeight, Global, Hsla, Pixels, Rgba, Styled, point, px};

pub use tokens::*;

/// A colour of `tokens.json`, light and dark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeColor {
    pub light: u32,
    pub dark: u32,
}

/// A number of `tokens.json`, light and dark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeNumber {
    pub light: f32,
    pub dark: f32,
}

impl ThemeNumber {
    pub fn value(self, dark: bool) -> f32 {
        if dark { self.dark } else { self.light }
    }
}

impl ThemeColor {
    pub fn hsla(self, dark: bool) -> Hsla {
        let hex = if dark { self.dark } else { self.light };
        Rgba { r: channel(hex >> 16), g: channel(hex >> 8), b: channel(hex), a: 1.0 }.into()
    }
}

fn channel(value: u32) -> f32 {
    (value & 0xff) as f32 / 255.0
}

/// Whether the app is drawn dark: the system's appearance, or the one picked in the settings.
#[derive(Default)]
pub struct Look {
    pub dark: bool,
}

impl Global for Look {}

/// The colours for the appearance the app is in, from anything that reaches the app.
pub trait Paint {
    fn dark(&self) -> bool;

    fn color(&self, color: ThemeColor) -> Hsla {
        color.hsla(self.dark())
    }

    /// The colour at one of the opacities: `primary` at `lit`.
    fn tint(&self, color: ThemeColor, opacity: ThemeNumber) -> Hsla {
        self.color(color).opacity(opacity.value(self.dark()))
    }

    fn number(&self, number: ThemeNumber) -> f32 {
        number.value(self.dark())
    }

    /// A shadow of the size, in the shadow colour at one of its opacities.
    fn shadow(&self, size: ShadowSize, strength: ThemeNumber) -> Vec<BoxShadow> {
        vec![BoxShadow {
            color: self.tint(SHADOW, strength),
            offset: point(px(0.), px(size.down())),
            blur_radius: px(size.blur()),
            spread_radius: px(0.),
            inset: false,
        }]
    }
}

impl Paint for App {
    fn dark(&self) -> bool {
        self.try_global::<Look>().is_some_and(|look| look.dark)
    }
}

/// The distances between things.
pub mod space {
    /// A label and its field, a control's edge and its contents.
    pub const XXS: f32 = 4.0;
    /// Inside a control: an icon and its words.
    pub const XS: f32 = 6.0;
    /// Between controls on a line.
    pub const S: f32 = 8.0;
    /// Between the lines of a block.
    pub const M: f32 = 12.0;
    /// Between blocks.
    pub const L: f32 = 16.0;
    /// A page's inset.
    pub const XL: f32 = 20.0;
    /// Between the sections of a page.
    pub const XXL: f32 = 28.0;
}

/// The sizes the app is drawn with, as the Mac app has them.
pub mod size {
    use super::space;

    pub const HAIRLINE: f32 = 1.0;
    /// Between the faces of buttons side by side, padded half inside each hit area.
    pub const BUTTON_GAP: f32 = 6.0;
    /// Between the faces of rows one above the other.
    pub const ROW_GAP: f32 = 2.0;
    pub const SMALL: f32 = 28.0;
    pub const REGULAR: f32 = 36.0;
    pub const LARGE: f32 = 44.0;
    pub const FIELD: f32 = REGULAR;
    pub const ROW: f32 = REGULAR;
    pub const SETTINGS_ROW: f32 = 52.0;
    pub const OPTION: f32 = 56.0;
    pub const FORM_WIDTH: f32 = 360.0;
    pub const SETTINGS_FIELD_WIDTH: f32 = 240.0;
    pub const CHIP: f32 = SMALL;
    pub const MESSAGE_AVATAR: f32 = 34.0;
    pub const ATTACHMENT_WIDTH: f32 = 168.0;
    pub const ATTACHMENT_PREVIEW: f32 = 96.0;
    pub const BAR: f32 = 52.0;
    pub const SEARCH: f32 = LARGE;
    pub const POPUP_WIDTH: f32 = 380.0;
    pub const SIDEBAR_END: f32 = 48.0;
    pub const ROW_HEIGHT: f32 = 48.0;
    pub const ACCOUNT_BAR: f32 = 2.0;
    pub const SELECT_GUTTER: f32 = 36.0;
    pub const SELECT_CIRCLE: f32 = 18.0;
    pub const SELECT_LINE: f32 = 1.5;
    /// The widest the list and the thread's card grow.
    pub const CARD_WIDTH: f32 = 1000.0;
    pub const CARD_INSET: f32 = 80.0;
    pub const CARD_MARGIN_X: f32 = space::XL + 4.0;
    pub const CARD_MARGIN_Y: f32 = space::L;
    pub const SIDEBAR_WIDTH: f32 = 300.0;
    pub const SETTINGS_SIDEBAR_MIN: f32 = 200.0;
    pub const SETTINGS_SIDEBAR_MAX: f32 = 360.0;
    pub const SETTINGS_PAGE: f32 = 640.0;
    /// The width a divider is grabbed by, over its hairline.
    pub const DIVIDER_GRAB: f32 = 9.0;
    pub const BANNER_WIDTH: f32 = 400.0;
    /// The body lines up with the sender's name beside the avatar.
    pub const BODY_INDENT: f32 = 46.0;
    /// A wide window shows the sender clicked beside the thread.
    pub const PANE_FROM: f32 = 1100.0;
    pub const PANE_WIDTH: f32 = 280.0;
}

/// The control heights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ControlSize {
    Small,
    #[default]
    Regular,
    Large,
}

impl ControlSize {
    pub fn height(self) -> f32 {
        match self {
            Self::Small => size::SMALL,
            Self::Regular => size::REGULAR,
            Self::Large => size::LARGE,
        }
    }

    pub fn icon(self) -> f32 {
        match self {
            Self::Small => 13.0,
            Self::Regular => 15.0,
            Self::Large => 17.0,
        }
    }

    pub fn text(self) -> TextStyle {
        match self {
            Self::Small | Self::Regular => TextStyle::LabelStrong,
            Self::Large => TextStyle::BodyStrong,
        }
    }
}

/// The face the app is drawn in, and the icons'.
pub const FONT: &str = "Nunito Sans";
pub const ICON_FONT: &str = "lucide";

/// Every size of text, with the weight and the colour it usually has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyle {
    /// The app's name on the first screen.
    Display,
    /// A thread's subject.
    Title,
    /// A sheet's or a pane's heading.
    Heading,
    /// A heading inside a page: a message's sender.
    Subheading,
    /// Mail, fields and rows.
    Body,
    /// Body text that stands out.
    BodyStrong,
    /// Controls, bars and the words beside them.
    Label,
    /// A control's words when they must stand out.
    LabelStrong,
    /// A line under a title: a detail, a date.
    Caption,
    /// The smallest words: a count, a field's label.
    Footnote,
    FootnoteStrong,
    /// Keys and codes.
    Mono,
}

impl TextStyle {
    pub fn size(self) -> f32 {
        match self {
            Self::Display => 28.0,
            Self::Title => 24.0,
            Self::Heading => 17.0,
            Self::Subheading | Self::Body | Self::BodyStrong => 14.0,
            Self::Label | Self::LabelStrong => 13.0,
            Self::Caption | Self::Mono => 12.0,
            Self::Footnote | Self::FootnoteStrong => 11.0,
        }
    }

    pub fn weight(self) -> FontWeight {
        match self {
            Self::Display | Self::Title | Self::Heading | Self::Subheading => FontWeight::SEMIBOLD,
            Self::BodyStrong | Self::LabelStrong => FontWeight::SEMIBOLD,
            Self::FootnoteStrong => FontWeight::BOLD,
            _ => FontWeight::NORMAL,
        }
    }

    pub fn color(self) -> ThemeColor {
        match self {
            Self::Caption | Self::Footnote | Self::FootnoteStrong => MUTED_STRONGER_FOREGROUND,
            Self::Mono => MUTED_FOREGROUND,
            _ => FOREGROUND,
        }
    }

    pub fn line_height(self) -> Pixels {
        px((self.size() * 1.36).round())
    }
}

/// Text set in a style of the scale.
pub trait Typeset: Styled + Sized {
    fn text(self, style: TextStyle, cx: &App) -> Self {
        self.text_in(style, style.color(), cx)
    }

    fn text_in(self, style: TextStyle, color: ThemeColor, cx: &App) -> Self {
        self.text_with(style, cx.color(color))
    }

    fn text_with(self, style: TextStyle, color: Hsla) -> Self {
        let family = if style == TextStyle::Mono { "monospace" } else { FONT };
        self.font_family(family)
            .text_size(px(style.size()))
            .line_height(style.line_height())
            .font_weight(style.weight())
            .text_color(color)
    }
}

impl<T: Styled> Typeset for T {}

/// The account colours, by the names the core gives them.
pub const ACCOUNT_COLORS: [ThemeColor; 10] =
    [ACCOUNT_1, ACCOUNT_2, ACCOUNT_3, ACCOUNT_4, ACCOUNT_5, ACCOUNT_6, ACCOUNT_7, ACCOUNT_8, ACCOUNT_9, ACCOUNT_10];

pub fn account_color(name: &str) -> ThemeColor {
    let number = name.strip_prefix("account-").and_then(|number| number.parse::<usize>().ok()).unwrap_or(1);
    ACCOUNT_COLORS[number.clamp(1, ACCOUNT_COLORS.len()) - 1]
}

pub fn account_color_names() -> Vec<String> {
    (1..=ACCOUNT_COLORS.len()).map(|number| format!("account-{number}")).collect()
}

/// A sender's colour, picked from their address as the Apple apps pick it.
pub fn sender_color(email: &str) -> ThemeColor {
    let sum = email.chars().fold(0u32, |sum, character| (sum.wrapping_mul(31).wrapping_add(character as u32)) & 0xffff);
    ACCOUNT_COLORS[sum as usize % ACCOUNT_COLORS.len()]
}

/// The card that fills the page and what rests on it.
pub const CARD_FILL: ThemeColor = CARD_LARGER;
pub const CARD_RULE: ThemeColor = BORDER_CARD;
pub const CARD_ACCENT: ThemeColor = CARD_LARGER_ACCENT;
pub const CARD_ACCENT_STRONGER: ThemeColor = CARD_LARGER_ACCENT_STRONGER;
