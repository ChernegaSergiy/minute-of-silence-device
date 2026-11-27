use esp_hal::gpio::{Input, InputConfig, InputPin, Pull};

const LONG_PRESS_MS: u64 = 1500;

/// Action detected from the hardware button.
pub enum ButtonEvent {
    None,
    Click,
    Pairing,
}

/// Hardware button handler with active-low logic (pull-up).
pub struct Button<'d> {
    input: Input<'d>,
    was_pressed: bool,
    pressed_at_ms: Option<u64>,
}

impl<'d> Button<'d> {
    /// Initializes a button input on the given pin with internal pull-up.
    pub fn new(pin: impl InputPin + 'd) -> Self {
        let config = InputConfig::default().with_pull(Pull::Up);
        let input = Input::new(pin, config);
        Self {
            input,
            was_pressed: false,
            pressed_at_ms: None,
        }
    }

    /// Updates button state and returns the completed action.
    pub fn update(&mut self, now_ms: u64) -> ButtonEvent {
        let is_pressed = self.input.is_low();
        let mut event = ButtonEvent::None;

        if is_pressed && !self.was_pressed {
            self.pressed_at_ms = Some(now_ms);
        } else if !is_pressed && self.was_pressed {
            if let Some(started_at) = self.pressed_at_ms.take() {
                event = if now_ms.saturating_sub(started_at) >= LONG_PRESS_MS {
                    crate::http_server::request_pairing();
                    ButtonEvent::Pairing
                } else {
                    ButtonEvent::Click
                };
            }
        }

        self.was_pressed = is_pressed;
        event
    }
}
