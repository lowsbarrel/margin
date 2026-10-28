use std::rc::Rc;

use gpui_kit::component::{Theme, ThemeConfig, ThemeMode, ThemeRegistry};
use gpui_kit::{App, Global, Window, WindowAppearance};

use crate::palette::Palette;
use crate::{fonts, kit_theme};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    #[must_use]
    pub fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("light") => Self::Light,
            Some("dark") => Self::Dark,
            _ => Self::System,
        }
    }

    #[must_use]
    pub const fn as_setting(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Debug)]
pub struct MarginTheme {
    appearance: Appearance,
    palette: Palette,
}

impl Global for MarginTheme {}

impl MarginTheme {
    pub fn init(appearance: Appearance, cx: &mut App) {
        fonts::register(cx);
        let registry = ThemeRegistry::global_mut(cx);
        registry
            .load_themes_from_str(&kit_theme::theme_set())
            .expect("the generated theme set matches the GPUI Kit schema");
        let light = registered(registry, kit_theme::LIGHT_NAME);
        let dark = registered(registry, kit_theme::DARK_NAME);
        Theme::update(cx, |theme| {
            theme.light_theme = light;
            theme.dark_theme = dark;
        });
        cx.set_global(Self {
            appearance,
            palette: Palette::light(),
        });
        Self::apply(cx);
    }

    #[must_use]
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    #[must_use]
    pub fn palette(cx: &App) -> &Palette {
        &cx.global::<Self>().palette
    }

    #[must_use]
    pub const fn appearance(&self) -> Appearance {
        self.appearance
    }

    pub fn set_appearance(appearance: Appearance, cx: &mut App) {
        cx.global_mut::<Self>().appearance = appearance;
        Self::apply(cx);
    }

    pub fn follow_system(window: &Window, cx: &mut App) {
        if cx.global::<Self>().appearance == Appearance::System {
            let dark = matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            );
            Self::apply_mode(dark, cx);
        }
    }

    fn apply(cx: &mut App) {
        let dark = match cx.global::<Self>().appearance {
            Appearance::Light => false,
            Appearance::Dark => true,
            Appearance::System => {
                matches!(
                    cx.window_appearance(),
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                )
            }
        };
        Self::apply_mode(dark, cx);
    }

    fn apply_mode(dark: bool, cx: &mut App) {
        cx.global_mut::<Self>().palette = if dark {
            Palette::dark()
        } else {
            Palette::light()
        };
        let mode = if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        Theme::change(mode, None, cx);
    }
}

fn registered(registry: &ThemeRegistry, name: &str) -> Rc<ThemeConfig> {
    registry
        .themes()
        .get(name)
        .cloned()
        .expect("Margin's themes were just loaded into the registry")
}
