use esp_hal::gpio::{Level, Output, OutputConfig, OutputPin};

use crate::led_pattern::CodeDisplay;
/// Status LED controller for board indicator (e.g. GPIO 2 onboard LED).
pub struct StatusLed<'d> {
    output: Output<'d>,
}

impl<'d> StatusLed<'d> {
    /// Initializes the status LED on the given output pin.
    pub fn new(pin: impl OutputPin + 'd) -> Self {
        let output = Output::new(pin, Level::Low, OutputConfig::default());
        Self { output }
    }

    /// Turns the status LED on.
    pub fn on(&mut self) {
        self.output.set_high();
    }

    /// Turns the status LED off.
    pub fn off(&mut self) {
        self.output.set_low();
    }

    /// Toggles the status LED state.
    pub fn toggle(&mut self) {
        self.output.toggle();
    }
}

/// Updates the pairing-code indicator without blocking the main loop.
pub fn show_code(
    led: &mut StatusLed<'_>,
    display: &mut CodeDisplay,
    code: Option<u32>,
    now_ms: u64,
) {
    if display.is_on(code, now_ms) {
        led.on();
    } else {
        led.off();
    }
}
