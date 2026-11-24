//! Colours, from loom's design tokens (loom/design/tokens.toml): near-monochrome, black for
//! what to press, one warm tone for what's changing. The only other colour is the tools' icons
//! in the rail. Light is the default; the viewer is black in both.

use gpui::{Rgba, rgb};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Light,
    Dark,
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub mode: Mode,
    pub bg: Rgba,
    /// Wells: the timeline, inner lists, the empty bin.
    pub side: Rgba,
    pub raised: Rgba,
    pub raised_2: Rgba,
    pub hover: Rgba,
    pub active: Rgba,
    pub line: Rgba,
    pub line_strong: Rgba,
    pub ink: Rgba,
    pub ink_2: Rgba,
    pub ink_3: Rgba,
    pub button: Rgba,
    pub button_ink: Rgba,
    pub ok: Rgba,
    pub bad: Rgba,
    pub warn: Rgba,
    pub viewer: Rgba,
    /// The tools' icons in the rail: edit, transcribe, convert, compress, download.
    pub tools: [Rgba; 5],
}

impl Theme {
    pub fn light() -> Self {
        Self {
            mode: Mode::Light,
            bg: rgb(0xffffff),
            side: rgb(0xf7f7f8),
            raised: rgb(0xf3f3f4),
            raised_2: rgb(0xececee),
            hover: rgb(0xf0f0f2),
            active: rgb(0xe8e8eb),
            line: rgb(0xebebed),
            line_strong: rgb(0xdcdce0),
            ink: rgb(0x111113),
            ink_2: rgb(0x4a4a52),
            ink_3: rgb(0x707079),
            button: rgb(0x111113),
            button_ink: rgb(0xffffff),
            ok: rgb(0x1f8a55),
            bad: rgb(0xd4442f),
            warn: rgb(0xc2621f),
            viewer: rgb(0x0b0b0c),
            tools: [rgb(0x11998e), rgb(0x1f8fc7), rgb(0xb8961a), rgb(0xcf4f9d), rgb(0x5763e0)],
        }
    }

    pub fn dark() -> Self {
        Self {
            mode: Mode::Dark,
            bg: rgb(0x0b0b0c),
            side: rgb(0x0f0f10),
            raised: rgb(0x161618),
            raised_2: rgb(0x1c1c1f),
            hover: rgb(0x18181b),
            active: rgb(0x202024),
            line: rgb(0x222226),
            line_strong: rgb(0x2e2e33),
            ink: rgb(0xededee),
            ink_2: rgb(0xb4b4ba),
            ink_3: rgb(0x86868f),
            button: rgb(0xededee),
            button_ink: rgb(0x0b0b0c),
            ok: rgb(0x6ccf9a),
            bad: rgb(0xff7a6b),
            warn: rgb(0xf2a77a),
            viewer: rgb(0x000000),
            tools: [rgb(0x6fd4c8), rgb(0x7cc8f0), rgb(0xe8d27a), rgb(0xe893c9), rgb(0x9ea8ff)],
        }
    }

    pub fn of(mode: Mode) -> Self {
        match mode {
            Mode::Light => Self::light(),
            Mode::Dark => Self::dark(),
        }
    }
}
