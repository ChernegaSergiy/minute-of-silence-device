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
