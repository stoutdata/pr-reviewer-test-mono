#![no_std]
#![no_main]

//! Product C ("voltmon") — nRF5340 application-core voltage-monitor firmware.
//!
//! One-shot samples SAADC channel AIN0 (P0.04) every 250 ms, converts the
//! 12-bit code to millivolts via the shared `monitor-config` crate under
//! `common/`, and logs each sample as plain text (`vmon <N> mv=<MV>`) on TWO
//! transports: RTT channel 0 (same mechanism as the blink products) and
//! UARTE0 TX at 115200 8N1. The product's lager test sweeps the monitored
//! rail with a bench supply and asserts the reported millivolts track it.
//!
//! UART TX pin: P1.05 — a free GPIO on the nRF5340-DK's P4 header, not
//! routed to any DK LED/button/VCOM function, so an external USB-UART
//! adapter can sit on it without fighting the interface MCU. Wire the
//! adapter's RX to P1.05 and share GND (see the repo README).
//!
//! Peripheral access: the app core boots in secure mode (the blink products
//! use the P0_S GPIO instance), but nrf5340-app-hal 0.19 hard-wires its
//! Saadc driver to the SAADC_NS instance and has no Port 1 pin types at all
//! on the app core, so SAADC and UARTE are driven through the secure PAC
//! instances directly (SAADC_S / UARTE0_S / P1_S). Polling only, no
//! interrupts.

use core::fmt::Write;
use core::sync::atomic::{compiler_fence, Ordering::SeqCst};

use cortex_m_rt::entry;
use embedded_hal::digital::OutputPin;
use nrf5340_app_hal as hal;
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

/// Additive output trim, millivolts, applied to every converted sample just
/// before it is reported. A calibrated unit needs none.
const SCALE_TRIM_MV: i32 = 0;

/// Sampling cadence: one SAADC conversion (and one log line) per period.
const SAMPLE_PERIOD_MS: u32 = 250;

/// UARTE0 TX = P1.05 (free pin on DK header P4; see module docs).
const UART_TX_PIN: usize = 5;

// The nRF5340 application core runs at 64 MHz out of reset.
const CPU_HZ: u32 = 64_000_000;

fn delay_ms(ms: u32) {
    cortex_m::asm::delay(CPU_HZ / 1000 * ms);
}

/// Fixed-size line buffer the UARTE transmits out of. Lives on the stack,
/// which is in RAM — a hard EasyDMA requirement (flash is not DMA-able).
struct Line {
    buf: [u8; 48],
    len: usize,
}

impl Line {
    const fn new() -> Self {
        Line { buf: [0; 48], len: 0 }
    }

    fn clear(&mut self) {
        self.len = 0;
    }

    fn bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl Write for Line {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &b in s.as_bytes() {
            if self.len == self.buf.len() {
                return Err(core::fmt::Error);
            }
            self.buf[self.len] = b;
            self.len += 1;
        }
        Ok(())
    }
}

/// One-time SAADC setup: 12-bit one-shot conversions of AIN0 (P0.04),
/// single-ended, gain 1/6 against the internal 0.6 V reference — full scale
/// 3.6 V, matching `monitor_config::FULL_SCALE_MV`. Ends with an offset
/// calibration pass.
fn saadc_init(saadc: &hal::pac::SAADC_S) {
    saadc.enable.write(|w| w.enable().enabled());
    saadc.resolution.write(|w| w.val()._12bit());
    saadc.oversample.write(|w| w.oversample().bypass());
    saadc.samplerate.write(|w| w.mode().task());

    saadc.ch[0].config.write(|w| {
        w.refsel().internal();
        w.gain().gain1_6();
        w.tacq()._10us();
        w.mode().se();
        w.resp().bypass();
        w.resn().bypass();
        w.burst().disabled();
        w
    });
    saadc.ch[0].pselp.write(|w| w.pselp().analog_input0());
    saadc.ch[0].pseln.write(|w| w.pseln().nc());

    saadc.events_calibratedone.reset();
    saadc.tasks_calibrateoffset.write(|w| unsafe { w.bits(1) });
    while saadc.events_calibratedone.read().bits() == 0 {}
}

/// One blocking one-shot conversion of channel 0. Returns the raw signed
/// SAADC code (single-ended offset error can dip slightly below zero).
fn saadc_sample(saadc: &hal::pac::SAADC_S) -> i16 {
    let mut code: i16 = 0;
    saadc
        .result
        .ptr
        .write(|w| unsafe { w.ptr().bits(&mut code as *mut i16 as u32) });
    saadc.result.maxcnt.write(|w| unsafe { w.maxcnt().bits(1) });

    compiler_fence(SeqCst);

    saadc.events_started.reset();
    saadc.tasks_start.write(|w| unsafe { w.bits(1) });
    while saadc.events_started.read().bits() == 0 {}

    saadc.events_end.reset();
    saadc.tasks_sample.write(|w| unsafe { w.bits(1) });
    while saadc.events_end.read().bits() == 0 {}

    saadc.events_stopped.reset();
    saadc.tasks_stop.write(|w| unsafe { w.bits(1) });
    while saadc.events_stopped.read().bits() == 0 {}

    compiler_fence(SeqCst);
    code
}

/// One-time UARTE0 setup: TX-only on P1.05, 115200 8N1, no flow control.
fn uarte_init(uarte: &hal::pac::UARTE0_S, p1: &hal::pac::P1_S) {
    // Park the TX pin high (UART idle) before handing it to the UARTE.
    p1.outset.write(|w| unsafe { w.bits(1 << UART_TX_PIN) });
    p1.pin_cnf[UART_TX_PIN].write(|w| {
        w.dir().output();
        w.input().disconnect();
        w.pull().disabled();
        w.drive().s0s1();
        w.sense().disabled();
        w
    });

    // PSEL.TXD: port 1, pin 5, connected. Everything else stays off — this
    // is a transmit-only console.
    uarte.psel.txd.write(|w| {
        unsafe { w.pin().bits(UART_TX_PIN as u8) };
        w.port().set_bit();
        w.connect().connected()
    });
    uarte.psel.rxd.write(|w| w.connect().disconnected());
    uarte.psel.cts.write(|w| w.connect().disconnected());
    uarte.psel.rts.write(|w| w.connect().disconnected());

    uarte
        .config
        .write(|w| w.hwfc().disabled().parity().excluded());
    uarte.baudrate.write(|w| w.baudrate().baud115200());
    uarte.enable.write(|w| w.enable().enabled());
}

/// Blocking EasyDMA transmit of `bytes` (which must be in RAM).
fn uarte_tx(uarte: &hal::pac::UARTE0_S, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    uarte
        .txd
        .ptr
        .write(|w| unsafe { w.ptr().bits(bytes.as_ptr() as u32) });
    uarte
        .txd
        .maxcnt
        .write(|w| unsafe { w.maxcnt().bits(bytes.len() as u16) });

    compiler_fence(SeqCst);

    uarte.events_endtx.reset();
    uarte.tasks_starttx.write(|w| unsafe { w.bits(1) });
    while uarte.events_endtx.read().bits() == 0 {}

    compiler_fence(SeqCst);
}

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::pac::Peripherals::take().unwrap();
    // Secure GPIO port 0 (the app core boots in secure mode).
    let port0 = hal::gpio::p0_s::Parts::new(p.P0_S);
    // nRF5340-DK LED3 = P0.30 (cosmetic heartbeat; the test asserts on the
    // reported millivolts, not the pin).
    let mut led = port0.p0_30.into_push_pull_output(hal::gpio::Level::High);

    saadc_init(&p.SAADC_S);
    uarte_init(&p.UARTE0_S, &p.P1_S);

    rprintln!("voltmon vmon demo start");

    let mut line = Line::new();
    let mut count: u32 = 0;
    loop {
        led.set_low().ok();

        let code = saadc_sample(&p.SAADC_S);
        // Clamp the slight negative codes a grounded input can produce.
        let code = if code < 0 { 0 } else { code as u16 };
        let mv = monitor_config::mv_from_code(code) as i32 + SCALE_TRIM_MV;
        let mv = if mv < 0 { 0 } else { mv as u32 };

        rprintln!("vmon {} mv={}", count, mv);
        line.clear();
        let _ = write!(line, "vmon {} mv={}\r\n", count, mv);
        uarte_tx(&p.UARTE0_S, line.bytes());

        led.set_high().ok();
        count = count.wrapping_add(1);
        delay_ms(SAMPLE_PERIOD_MS);
    }
}
