use esp_hal::gpio::{Input, InputConfig, InputPin, Pull};

/// Hardware button handler with active-low logic (pull-up).
pub struct Button<'d> {
    input: Input<'d>,
    was_pressed: bool,
}

impl<'d> Button<'d> {
    /// Initializes a button input on the given pin with internal pull-up.
    pub fn new(pin: impl InputPin + 'd) -> Self {
        let config = InputConfig::default().with_pull(Pull::Up);
        let input = Input::new(pin, config);
        Self {
            input,
            was_pressed: false,
        }
    }

    /// Checks if the button was clicked (active-low state transition).
    pub fn is_clicked(&mut self) -> bool {
        let is_pressed = self.input.is_low();
        let clicked = !is_pressed && self.was_pressed;
        self.was_pressed = is_pressed;
        clicked
    }
}
