#![no_std]

//! Shared SAADC-code-to-millivolts policy for the analog products in this
//! monorepo.
//!
//! `voltmon` (and any future product that samples the SAADC) derives its
//! reported millivolt values from this crate, so a change here affects every
//! analog product — the same role the shared `common/` path prefix plays for
//! `blink-config` (a PR touching `common/` triggers ALL product reviewers).

/// Full-scale input in millivolts for a single-ended SAADC channel using
/// gain 1/6 against the internal 0.6 V reference: 0.6 V / (1/6) = 3.6 V.
pub const FULL_SCALE_MV: u32 = 3600;

/// Code span of a 12-bit conversion (2^12).
pub const CODE_SPAN: u32 = 4096;

/// Millivolts for a 12-bit SAADC code: `code * 3600 / 4096`.
pub const fn mv_from_code(code: u16) -> u32 {
    code as u32 * FULL_SCALE_MV / CODE_SPAN
}
