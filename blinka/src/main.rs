#![no_std]
#![no_main]

//! Product A ("blinka") — nRF5340 application-core blink firmware.
//!
//! Toggles an LED and logs each toggle over RTT channel 0 as plain text
//! (`blink <N> period=<MS>ms`) so the product's lager test can measure the
//! blink interval by reading the RTT channel. The printed `period=` value is
//! the source of truth for the test; the physical LED toggle is cosmetic.
//!
//! The period is derived from the shared `blink-config` crate under
//! `common/` — a change there affects every product in the monorepo.

use cortex_m_rt::entry;
use embedded_hal::digital::OutputPin;
use nrf5340_app_hal as hal;
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

// Width of a blink-period value. Defined here on `main` (the PR base) so it
// never appears in a feature PR's diff hunk. A period value routed through a
// too-narrow alias is silently truncated at runtime (e.g. `700u16 as u8` ==
// 188) — the latent trap a closed-loop fw_bug demo exercises. The baseline
// below prints the period directly, so the alias is dormant.
#[allow(dead_code)]
type BlinkVal = u8;

/// This product blinks at 5x the shared base period (500 ms).
const PERIOD_MULT: u16 = 5;
const BLINK_PERIOD_MS: u16 = blink_config::period_ms(PERIOD_MULT);

// The nRF5340 application core runs at 64 MHz out of reset.
const CPU_HZ: u32 = 64_000_000;

fn delay_ms(ms: u32) {
    cortex_m::asm::delay(CPU_HZ / 1000 * ms);
}

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::pac::Peripherals::take().unwrap();
    // Secure GPIO port 0 (the app core boots in secure mode).
    let port0 = hal::gpio::p0_s::Parts::new(p.P0_S);
    // nRF5340-DK LED1 = P0.28 (cosmetic; the test asserts on RTT, not the pin).
    let mut led = port0.p0_28.into_push_pull_output(hal::gpio::Level::High);

    rprintln!("blinka blink demo start");

    let mut count: u32 = 0;
    loop {
        led.set_low().ok();
        rprintln!("blink {} period={}ms", count, BLINK_PERIOD_MS);
        delay_ms(BLINK_PERIOD_MS as u32);
        led.set_high().ok();
        count = count.wrapping_add(1);
    }
}
