use std::fs::OpenOptions;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;
use rand::Rng;

/// Introduces a random delay to make the bot behavior less predictable
/// This might helps avoid detection by anti-cheat systems
fn random_delay(min_delay: u64, max_delay: u64) {
    let mut rng = rand::rng();
    let delay_ms = rng.random_range(min_delay..=max_delay);

    sleep(Duration::from_millis(delay_ms))
}

pub enum KeyAction {
    Up,
    Down,
    Tap,
}

/// HID keyboard interface for sending keystrokes through /dev/hidg0
/// Useable for linux usb gadget HID mode.
pub struct HidKeyboard {
    device: std::fs::File,
}

impl HidKeyboard {
    /// Creates a new HidKeyboard by opening the HID gadget device
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let device = OpenOptions::new().write(true).open("/dev/hidg0")?;
        Ok(Self { device })
    }

    /// Sends a key press event for the specified key code and action type
    pub fn key(&mut self, key_code: u8, action: KeyAction) -> Result<(), Box<dyn std::error::Error>> {
        match action {
            KeyAction::Down => {
                self.device.write_all(&[0, 0, key_code, 0, 0, 0, 0, 0])?;
            }
            KeyAction::Up => {
                self.device.write_all(&[0, 0, 0, 0, 0, 0, 0, 0])?;
            }
            KeyAction::Tap => {
                self.device.write_all(&[0, 0, key_code, 0, 0, 0, 0, 0])?;
                random_delay(50, 150);
                self.device.write_all(&[0, 0, 0, 0, 0, 0, 0, 0])?;
            }
        }

        Ok(())
    }
}