use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    AnimationExt, ElementId, Hsla, IntoElement, ParentElement, Styled, canvas, div, rems,
};
use margin_theme::Palette;

use crate::art_scenes::{files, passphrase, sync};
use crate::art_scenes_more::{caret, find, history};
use crate::sketch::{Ink, Sketch, Viewport};
use crate::timeline::{Timeline, entrance};

pub(crate) const VIEW_WIDTH: f32 = 200.0;
pub(crate) const VIEW_HEIGHT: f32 = 130.0;
const ART_WIDTH_REMS: f32 = 15.7143;
const ART_HEIGHT_REMS: f32 = 10.7143;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IntroStep {
    Files,
    Passphrase,
    Sync,
    Find,
    History,
}

impl IntroStep {
    pub(crate) const ALL: [Self; 5] = [
        Self::Files,
        Self::Passphrase,
        Self::Sync,
        Self::Find,
        Self::History,
    ];

    pub(crate) const fn title_key(self) -> &'static str {
        match self {
            Self::Files => "onboarding_files_title",
            Self::Passphrase => "onboarding_passphrase_title",
            Self::Sync => "onboarding_sync_title",
            Self::Find => "onboarding_find_title",
            Self::History => "onboarding_history_title",
        }
    }

    pub(crate) const fn body_key(self) -> &'static str {
        match self {
            Self::Files => "onboarding_files_body",
            Self::Passphrase => "onboarding_passphrase_body",
            Self::Sync => "onboarding_sync_body",
            Self::Find => "onboarding_find_body",
            Self::History => "onboarding_history_body",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Colors {
    pub page: Hsla,
    pub raised: Hsla,
    pub line: Ink,
    pub brand: Hsla,
    pub muted: Hsla,
    pub divider: Hsla,
    pub brand_strong: Hsla,
    pub brand_soft: Hsla,
}

impl Colors {
    fn of(palette: &Palette) -> Self {
        Self {
            page: palette.surface(0),
            raised: palette.surface(1),
            line: Ink::new(palette.border_strong, 1.5),
            brand: palette.text_brand,
            muted: palette.text_tertiary,
            divider: palette.border_secondary,
            brand_strong: palette.brand(24.0),
            brand_soft: palette.brand(16.0),
        }
    }

    pub(crate) const fn brand_ink(self, width: f32) -> Ink {
        Ink::new(self.brand, width)
    }

    pub(crate) const fn muted_ink(self) -> Ink {
        Ink::new(self.muted, 2.0)
    }
}

fn scene(step: IntroStep, colors: Colors, progress: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let viewport = Viewport::fit(bounds, VIEW_WIDTH, VIEW_HEIGHT);
            let mut sketch = Sketch::new(viewport, window);
            let at = Timeline::at(progress);
            match step {
                IntroStep::Files => files(&mut sketch, colors, at),
                IntroStep::Passphrase => passphrase(&mut sketch, colors, at),
                IntroStep::Sync => sync(&mut sketch, colors, at),
                IntroStep::Find => find(&mut sketch, colors, at),
                IntroStep::History => history(&mut sketch, colors, at),
            }
        },
    )
    .size_full()
}

pub(crate) fn intro_art(step: IntroStep, palette: &Palette) -> impl IntoElement {
    let colors = Colors::of(palette);
    let id = ElementId::from(("intro-art", step as usize));
    div()
        .relative()
        .w(rems(ART_WIDTH_REMS))
        .h(rems(ART_HEIGHT_REMS))
        .child(
            div()
                .size_full()
                .with_animation(id, entrance(), move |art, progress| {
                    art.child(scene(step, colors, progress))
                }),
        )
        .when(step == IntroStep::Find, |art| art.child(caret(colors)))
}
