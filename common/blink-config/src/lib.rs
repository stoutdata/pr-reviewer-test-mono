#![no_std]

//! Shared blink-period policy for the products in this monorepo.
//!
//! Both `blinka` and `blinkb` derive their blink period from this crate, so a
//! change here affects every product — which is exactly what the shared
//! `common/` path prefix in each product's PR-reviewer config models (a PR
//! touching `common/` triggers ALL product reviewers).

/// Base period every product multiplies. Milliseconds.
pub const BASE_PERIOD_MS: u16 = 100;

/// A product's blink period: its multiplier times the shared base.
pub const fn period_ms(multiplier: u16) -> u16 {
    BASE_PERIOD_MS * multiplier
}

/// Half of a product's period — a convenience for diagnostics that sample
/// at twice the blink rate.
pub const fn half_period_ms(multiplier: u16) -> u16 {
    period_ms(multiplier) / 2
}
