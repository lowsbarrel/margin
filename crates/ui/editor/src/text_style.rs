use std::ops::Range;
use std::sync::Arc;

use gpui_kit::{
    Font, FontFeatures, FontStyle, FontWeight, Hsla, StrikethroughStyle, TextRun, UnderlineStyle,
    px,
};
use margin_live_preview::{HighlightColor, InlineStyle, LineView, LinkKind, SyntaxRole};
use margin_theme::{HighlightTone, MONO_FONT_FAMILY, Palette, SyntaxTone, UI_FONT_FAMILY};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Face {
    Body,
    Mono,
}

pub(crate) fn font(face: Face) -> Font {
    match face {
        Face::Body => Font {
            family: UI_FONT_FAMILY.into(),
            features: FontFeatures(Arc::new(vec![
                ("dlig".to_owned(), 1),
                ("ss07".to_owned(), 1),
                ("calt".to_owned(), 0),
            ])),
            fallbacks: None,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
        },
        Face::Mono => Font {
            family: MONO_FONT_FAMILY.into(),
            features: FontFeatures::default(),
            fallbacks: None,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
        },
    }
}

pub(crate) const fn syntax_tone(role: SyntaxRole) -> SyntaxTone {
    match role {
        SyntaxRole::Keyword => SyntaxTone::Keyword,
        SyntaxRole::String => SyntaxTone::String,
        SyntaxRole::Number => SyntaxTone::Number,
        SyntaxRole::Function => SyntaxTone::Function,
        SyntaxRole::Comment => SyntaxTone::Comment,
        SyntaxRole::Attribute => SyntaxTone::Attribute,
    }
}

const fn highlight_tone(color: HighlightColor) -> HighlightTone {
    match color {
        HighlightColor::Yellow => HighlightTone::Yellow,
        HighlightColor::Blue => HighlightTone::Blue,
        HighlightColor::Green => HighlightTone::Green,
        HighlightColor::Purple => HighlightTone::Purple,
        HighlightColor::Red => HighlightTone::Red,
        HighlightColor::Orange => HighlightTone::Orange,
    }
}

pub(crate) struct RunContext<'a> {
    pub palette: &'a Palette,
    pub face: Face,
    pub color: Hsla,
    pub missing: &'a dyn Fn(&LinkKind) -> bool,
    pub live: bool,
}

pub(crate) fn inline_runs(
    view: &LineView,
    range: Range<usize>,
    context: &RunContext<'_>,
) -> Vec<TextRun> {
    let mut runs = Vec::new();
    let mut at = range.start;
    for styled in &view.runs {
        let start = styled.range.start.max(range.start);
        let end = styled.range.end.min(range.end);
        if start >= end {
            continue;
        }
        if start > at {
            runs.push(plain_run(start - at, context));
        }
        let missing = view.links.iter().any(|link| {
            link.display.start < end && start < link.display.end && (context.missing)(&link.kind)
        });
        runs.push(styled_run(&styled.style, end - start, missing, context));
        at = end;
    }
    if range.end > at {
        runs.push(plain_run(range.end - at, context));
    }
    runs
}

pub(crate) fn code_runs(
    text_len: usize,
    colors: &[(Range<usize>, SyntaxRole)],
    context: &RunContext<'_>,
) -> Vec<TextRun> {
    let mut runs = Vec::new();
    let mut at = 0;
    for (range, role) in colors {
        let start = range.start.max(at);
        let end = range.end.min(text_len);
        if start >= end {
            continue;
        }
        if start > at {
            runs.push(plain_run(start - at, context));
        }
        let mut run = plain_run(end - start, context);
        run.color = context.palette.syntax(syntax_tone(*role));
        runs.push(run);
        at = end;
    }
    if text_len > at {
        runs.push(plain_run(text_len - at, context));
    }
    runs
}

fn plain_run(len: usize, context: &RunContext<'_>) -> TextRun {
    TextRun {
        len,
        font: font(context.face),
        color: context.color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

fn styled_run(style: &InlineStyle, len: usize, missing: bool, context: &RunContext<'_>) -> TextRun {
    let palette = context.palette;
    let mut font = if style.code || style.keyboard {
        font(Face::Mono)
    } else {
        font(context.face)
    };
    if style.bold || style.heading.is_some() {
        font.weight = FontWeight::SEMIBOLD;
    } else if style.link && !missing && context.live {
        font.weight = FontWeight::MEDIUM;
    }
    if style.italic {
        font.style = FontStyle::Italic;
    }
    let mut color = if style.markup || style.done || style.link && missing {
        palette.text_tertiary
    } else if style.link || style.tag {
        palette.text_brand
    } else if let Some(role) = style.syntax {
        palette.syntax(syntax_tone(role))
    } else if style.code {
        palette.text_code
    } else if style.quote {
        palette.text_secondary
    } else {
        context.color
    };
    if style.ornament {
        color = color.opacity(0.6);
    } else if style.strike && context.live {
        color = color.opacity(0.5);
    }
    let background_color = style
        .highlight
        .map(|tone| palette.highlight(highlight_tone(tone)))
        .or_else(|| (style.code || style.keyboard).then(|| palette.surface(2)))
        .or_else(|| (style.tag && context.live).then(|| palette.brand(12.0)));
    TextRun {
        len,
        font,
        color,
        background_color,
        underline: style.underline.then_some(UnderlineStyle {
            thickness: px(1.0),
            color: None,
            wavy: false,
        }),
        strikethrough: (style.strike || style.done).then_some(StrikethroughStyle {
            thickness: px(1.0),
            color: None,
        }),
    }
}
