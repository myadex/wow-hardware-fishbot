use rand::Rng;
use std::fs::OpenOptions;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

/// Introduces a random delay to make the bot behavior less predictable
/// This might helps avoid detection by anti-cheat systems
fn random_delay(min_delay: u64, max_delay: u64) {
    let mut rng = rand::rng();
    let delay_ms = rng.random_range(min_delay..=max_delay);

    sleep(Duration::from_millis(delay_ms))
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

    /// Presses and releases a key through the keyboard HID gadget.
    pub fn tap(&mut self, key_code: u8) -> Result<(), Box<dyn std::error::Error>> {
        self.device.write_all(&[0, 0, key_code, 0, 0, 0, 0, 0])?;
        random_delay(50, 150);
        self.device.write_all(&[0, 0, 0, 0, 0, 0, 0, 0])?;
        Ok(())
    }
}
