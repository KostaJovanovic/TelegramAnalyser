//! The Swiss/International design system, in GPUI.
//!
//! Ported from `telegram_rust/crates/tgx-ui`. The tokens are the same values
//! `tga_report::palette` writes into the report's stylesheet, so the window and
//! the file it produces are visibly one product — `tokens::tests` is what stops
//! the two copies drifting.
//!
//! No I/O, no export model, no report. This crate paints.

pub mod components;
pub mod fonts;
pub mod tokens;
