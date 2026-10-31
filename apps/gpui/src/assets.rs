//! The files the app draws with: its own SVGs, the fonts, and GPUI Component's icons.

use std::borrow::Cow;

use gpui::{App, AssetSource, Result, SharedString};

pub struct Assets;

const OWN: [(&str, &[u8]); 11] = [
    ("wonnet/star-filled.svg", include_bytes!("../assets/wonnet/star-filled.svg")),
    ("wonnet/loader.svg", include_bytes!("../assets/wonnet/loader.svg")),
    ("wonnet/app-mark.svg", include_bytes!("../assets/wonnet/app-mark.svg")),
    ("wonnet/google-blue.svg", include_bytes!("../assets/wonnet/google-blue.svg")),
    ("wonnet/google-green.svg", include_bytes!("../assets/wonnet/google-green.svg")),
    ("wonnet/google-yellow.svg", include_bytes!("../assets/wonnet/google-yellow.svg")),
    ("wonnet/google-red.svg", include_bytes!("../assets/wonnet/google-red.svg")),
    ("wonnet/microsoft-red.svg", include_bytes!("../assets/wonnet/microsoft-red.svg")),
    ("wonnet/microsoft-green.svg", include_bytes!("../assets/wonnet/microsoft-green.svg")),
    ("wonnet/microsoft-blue.svg", include_bytes!("../assets/wonnet/microsoft-blue.svg")),
    ("wonnet/microsoft-yellow.svg", include_bytes!("../assets/wonnet/microsoft-yellow.svg")),
];

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = OWN.iter().find(|(name, _)| *name == path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_kit_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut found: Vec<SharedString> =
            OWN.iter().filter(|(name, _)| name.starts_with(path)).map(|(name, _)| SharedString::from(*name)).collect();
        found.extend(gpui_kit_assets::Assets.list(path)?);
        Ok(found)
    }
}

/// Puts the app's face and Lucide's in the text system.
pub fn load_fonts(cx: &mut App) {
    let fonts: Vec<Cow<'static, [u8]>> = vec![
        Cow::Borrowed(include_bytes!("../assets/fonts/NunitoSans-400.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/NunitoSans-600.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/NunitoSans-700.ttf")),
        Cow::Borrowed(include_bytes!("../../../packages/apple/Sources/MailUI/Fonts/lucide.ttf")),
    ];
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        eprintln!("The fonts couldn't load: {error:#}");
    }
}
