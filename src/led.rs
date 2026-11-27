use embassy_time::{Duration, Timer};
use esp_hal::gpio::{Level, Output, OutputConfig, OutputPin};

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

/// Shows one digit of the current pairing code and advances the display phase.
pub async fn show_code<'d>(led: &mut StatusLed<'d>, code: Option<u32>, phase: &mut u8) {
    let Some(code) = code else {
        led.on();
        *phase = 0;
        return;
    };

    let mut digits = [0u8; 6];
    let mut value = code;
    for digit in digits.iter_mut().rev() {
        *digit = (value % 10) as u8;
        value /= 10;
    }

    let slot = usize::from(*phase / 3) % 7;
    *phase = phase.wrapping_add(1);

    if slot == 6 {
        led.off();
        Timer::after(Duration::from_millis(150)).await;
        return;
    }

    let digit = digits[slot];
    if digit == 0 {
        led.on();
        Timer::after(Duration::from_millis(500)).await;
        led.off();
    } else {
        for _ in 0..digit {
            led.on();
            Timer::after(Duration::from_millis(80)).await;
            led.off();
            Timer::after(Duration::from_millis(80)).await;
        }
    }
    Timer::after(Duration::from_millis(150)).await;
}
