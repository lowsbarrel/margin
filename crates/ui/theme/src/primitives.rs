use gpui_kit::{Hsla, hsla};

pub(crate) fn hsl(hue: f32, saturation: f32, lightness: f32) -> Hsla {
    hsla(hue / 360.0, saturation / 100.0, lightness / 100.0, 1.0)
}

pub(crate) fn hsl_alpha(hue: f32, saturation: f32, lightness: f32, alpha: f32) -> Hsla {
    hsla(
        hue / 360.0,
        saturation / 100.0,
        lightness / 100.0,
        alpha / 100.0,
    )
}

pub(crate) fn neutral(step: u8) -> Hsla {
    let lightness = match step {
        0 => 100.0,
        5 => 96.0,
        10 => 93.0,
        20 => 86.0,
        30 => 76.0,
        40 => 68.0,
        50 => 53.0,
        60 => 44.0,
        70 => 36.0,
        80 => 25.0,
        90 => 15.0,
        95 => 12.0,
        _ => 8.0,
    };
    hsl(0.0, 0.0, lightness)
}

pub(crate) fn orange(step: u8) -> Hsla {
    match step {
        30 => hsl(18.0, 100.0, 78.0),
        40 => hsl(16.0, 100.0, 70.0),
        50 => hsl(15.0, 100.0, 60.0),
        60 => hsl(15.0, 94.0, 52.0),
        _ => hsl(15.0, 96.0, 45.0),
    }
}

pub(crate) fn green(step: u8) -> Hsla {
    match step {
        40 => hsl(112.0, 45.0, 52.0),
        _ => hsl(107.0, 88.0, 26.0),
    }
}

pub(crate) fn yellow(step: u8) -> Hsla {
    match step {
        30 => hsl(42.0, 100.0, 64.0),
        40 => hsl(40.0, 96.0, 52.0),
        60 => hsl(36.0, 100.0, 33.0),
        _ => hsl(34.0, 100.0, 27.0),
    }
}

pub(crate) fn red(step: u8) -> Hsla {
    match step {
        30 => hsl(9.0, 100.0, 75.0),
        40 => hsl(10.0, 96.0, 64.0),
        60 => hsl(12.0, 94.0, 47.0),
        _ => hsl(12.0, 96.0, 41.0),
    }
}

pub(crate) fn purple(step: u8) -> Hsla {
    match step {
        40 => hsl(276.0, 80.0, 68.0),
        _ => hsl(275.0, 69.0, 49.0),
    }
}

pub(crate) fn blue(step: u8) -> Hsla {
    match step {
        40 => hsl(216.0, 100.0, 64.0),
        80 => hsl(216.0, 100.0, 40.0),
        _ => hsl(216.0, 100.0, 50.0),
    }
}

pub(crate) fn cyan(step: u8) -> Hsla {
    match step {
        40 => hsl(179.0, 68.0, 60.0),
        _ => hsl(179.0, 100.0, 37.0),
    }
}
