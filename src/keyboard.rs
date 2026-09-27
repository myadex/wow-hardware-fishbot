use rand::Rng;
use std::fs::OpenOptions;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

/// USB HID keyboard usage for a configurable bite-action key.
pub fn parse_key_name(name: &str) -> Result<u8, Box<dyn std::error::Error>> {
    let upper = name.trim().to_ascii_uppercase();
    let bytes = upper.as_bytes();
    if bytes.len() == 1 {
        return match bytes[0] {
            b'A'..=b'Z' => Ok(0x04 + bytes[0] - b'A'),
            b'1'..=b'9' => Ok(0x1e + bytes[0] - b'1'),
            b'0' => Ok(0x27),
            _ => Err("Bite key must be A-Z, 0-9 or F1-F12".into()),
        };
    }
    if let Some(number) = upper.strip_prefix('F').and_then(|n| n.parse::<u8>().ok()) {
        if (1..=12).contains(&number) {
            return Ok(0x3a + number - 1);
        }
    }
    Err("Bite key must be A-Z, 0-9 or F1-F12".into())
}

#[cfg(test)]
mod tests {
    use super::parse_key_name;

    #[test]
    fn parses_common_hid_keys_and_rejects_invalid_names() {
        assert_eq!(parse_key_name("e").unwrap(), 0x08);
        assert_eq!(parse_key_name("3").unwrap(), 0x20);
        assert_eq!(parse_key_name("0").unwrap(), 0x27);
        assert_eq!(parse_key_name("F8").unwrap(), 0x41);
        assert!(parse_key_name("F13").is_err());
        assert!(parse_key_name("mouse-right").is_err());
    }
}

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
