//! The icons, from Lucide's font (lucide-static 1.52.0, as the Apple apps have it): each is its
//! character in that version's `font/codepoints.json`. The filled star and the turning loader
//! are drawn from SVG.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, Div, Hsla, IntoElement, ParentElement, Styled, Transformation, div, percentage, px,
    svg,
};

use crate::theme::ICON_FONT;

/// Every icon the Apple apps have, though not all are drawn here yet.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Symbol {
    AlarmClock,
    Archive,
    ArchiveRestore,
    ArrowLeft,
    ArrowUp,
    AtSign,
    Ban,
    Bell,
    BellOff,
    CalendarClock,
    Check,
    CheckCheck,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronsDownUp,
    ChevronsUpDown,
    CircleAlert,
    CircleArrowDown,
    CircleCheck,
    Clock,
    Command,
    Download,
    Ellipsis,
    File,
    FileArchive,
    FileAudio,
    FileImage,
    FileSpreadsheet,
    FileText,
    FileVideo,
    FolderInput,
    Forward,
    Globe,
    Handshake,
    Image,
    Inbox,
    Keyboard,
    ListFilter,
    Loader,
    LogOut,
    Mail,
    MailMinus,
    MailOpen,
    MailX,
    Menu,
    Monitor,
    Moon,
    Paperclip,
    Pencil,
    Plus,
    Presentation,
    Printer,
    RefreshCw,
    Reply,
    ReplyAll,
    Search,
    Send,
    Server,
    Settings,
    ShieldAlert,
    SquareCheck,
    SquarePen,
    Star,
    /// A star filled in, drawn from SVG.
    StarFilled,
    Sun,
    Tag,
    Trash,
    Undo,
    User,
    WifiOff,
    X,
    Zap,
}

impl Symbol {
    pub fn character(self) -> char {
        let code = match self {
            Self::AlarmClock => 0xe03a,
            Self::Archive => 0xe041,
            Self::ArchiveRestore => 0xe2cd,
            Self::ArrowLeft => 0xe048,
            Self::ArrowUp => 0xe04a,
            Self::AtSign => 0xe04e,
            Self::Ban => 0xe051,
            Self::Bell => 0xe059,
            Self::BellOff => 0xe05a,
            Self::CalendarClock => 0xe304,
            Self::Check => 0xe06c,
            Self::CheckCheck => 0xe38e,
            Self::ChevronDown => 0xe06d,
            Self::ChevronLeft => 0xe06e,
            Self::ChevronRight => 0xe06f,
            Self::ChevronUp => 0xe070,
            Self::ChevronsDownUp => 0xe228,
            Self::ChevronsUpDown => 0xe211,
            Self::CircleAlert => 0xe077,
            Self::CircleArrowDown => 0xe078,
            Self::CircleCheck => 0xe226,
            Self::Clock => 0xe087,
            Self::Command => 0xe09a,
            Self::Download => 0xe0b2,
            Self::Ellipsis => 0xe0b6,
            Self::File => 0xe0c0,
            Self::FileArchive => 0xe30d,
            Self::FileAudio => 0xe31a,
            Self::FileImage => 0xe31c,
            Self::FileSpreadsheet => 0xe326,
            Self::FileText => 0xe0cc,
            Self::FileVideo => 0xe321,
            Self::FolderInput => 0xe334,
            Self::Forward => 0xe229,
            Self::Globe => 0xe0e8,
            Self::Handshake => 0xe5c0,
            Self::Image => 0xe0f6,
            Self::Inbox => 0xe0f7,
            Self::Keyboard => 0xe284,
            Self::ListFilter => 0xe460,
            Self::Loader => 0xe109,
            Self::LogOut => 0xe10e,
            Self::Mail => 0xe10f,
            Self::MailMinus => 0xe362,
            Self::MailOpen => 0xe363,
            Self::MailX => 0xe368,
            Self::Menu => 0xe115,
            Self::Monitor => 0xe11d,
            Self::Moon => 0xe11e,
            Self::Paperclip => 0xe12d,
            Self::Pencil => 0xe1f9,
            Self::Plus => 0xe13d,
            Self::Presentation => 0xe4ae,
            Self::Printer => 0xe141,
            Self::RefreshCw => 0xe145,
            Self::Reply => 0xe22a,
            Self::ReplyAll => 0xe22b,
            Self::Search => 0xe151,
            Self::Send => 0xe152,
            Self::Server => 0xe153,
            Self::Settings => 0xe154,
            Self::ShieldAlert => 0xe1fe,
            Self::SquareCheck => 0xe559,
            Self::SquarePen => 0xe172,
            Self::Star | Self::StarFilled => 0xe176,
            Self::Sun => 0xe178,
            Self::Tag => 0xe17f,
            Self::Trash => 0xe18e,
            Self::Undo => 0xe2a1,
            Self::User => 0xe19f,
            Self::WifiOff => 0xe1af,
            Self::X => 0xe1b2,
            Self::Zap => 0xe1b4,
        };
        char::from_u32(code).unwrap_or(' ')
    }

    /// The icon a mailbox is drawn with, by the name the core gives it.
    pub fn named(name: &str) -> Symbol {
        match name {
            "inbox" => Self::Inbox,
            "mail" => Self::Mail,
            "star" => Self::Star,
            "clock" => Self::Clock,
            "send" => Self::Send,
            "file" => Self::File,
            "archive" => Self::Archive,
            "shield-alert" => Self::ShieldAlert,
            "trash" => Self::Trash,
            _ => Self::Tag,
        }
    }
}

/// The icon `side` points square, in the colour of the text around it unless given one.
pub fn icon(symbol: Symbol, side: f32) -> Div {
    let frame = div().flex_none().size(px(side)).flex().items_center().justify_center();
    if symbol == Symbol::StarFilled {
        return frame.child(svg().path("wonnet/star-filled.svg").size(px(side)).text_color(gpui::transparent_black()));
    }
    frame.font_family(ICON_FONT).text_size(px(side)).line_height(px(side)).child(symbol.character().to_string())
}

/// The icon in a colour.
pub fn icon_in(symbol: Symbol, side: f32, color: Hsla) -> Div {
    if symbol == Symbol::StarFilled {
        let frame = div().flex_none().size(px(side)).flex().items_center().justify_center();
        return frame.child(svg().path("wonnet/star-filled.svg").size(px(side)).text_color(color));
    }
    icon(symbol, side).text_color(color)
}

/// Lucide's loader turning once a second, for what is still coming.
pub fn spinner(side: f32, color: Hsla) -> impl IntoElement + use<> {
    div().flex_none().size(px(side)).flex().items_center().justify_center().child(
        svg().path("wonnet/loader.svg").size(px(side)).text_color(color).with_animation(
            "spinner",
            Animation::new(Duration::from_secs(1)).repeat(),
            |svg, delta| svg.with_transformation(Transformation::rotate(percentage(delta))),
        ),
    )
}
