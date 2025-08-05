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

pub enum ButtonAction {
    Up,
    Down,
    Click,
}

pub enum Button{
    Left = 1,
    Right = 2,
    Middle = 4,
}

/// HID keyboard interface for sending keystrokes through /dev/hidg1
/// Useable for linux usb gadget HID mode.
pub struct HidMouse {
    device: std::fs::File,
    cursor_x: i32,
    cursor_y: i32,
}

impl HidMouse {
    /// Creates a new HidMouse by opening the HID gadget device
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let device = OpenOptions::new().write(true).open("/dev/hidg1")?;
        Ok(Self { 
            device,
            cursor_x: 0,
            cursor_y: 0,
        })
    }
    
    /// Moves thes cursor to the top left.
    pub fn cursor_home(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for _ in 0..20 {
            self.device.write_all(&[0, (-127i8) as u8, (-127i8) as u8, 0])?;
            random_delay(10, 25);
        }

        // Reset the internal cursor position tracking
        self.cursor_x = 0;
        self.cursor_y = 0;

        Ok(())
    }
    
    pub fn cursor_move(&mut self, target_x: i32, target_y: i32) -> Result<(), Box<dyn std::error::Error>> {
        let mut remaining_x: i32 = target_x - self.cursor_x;
        let mut remaining_y: i32 = target_y - self.cursor_y;

        while remaining_x != 0 || remaining_y != 0 {
            // Calculate how much to move in this step
            let step_x = remaining_x.clamp(-127, 127);
            let step_y = remaining_y.clamp(-127, 127);

            println!{"x: {} y: {}", step_x, step_y}
            
            // Send the movement - cast i32 to i8 first, then to u8
            self.device.write_all(&[0, step_x as i8 as u8, step_y as i8 as u8, 0])?;
            
            // Update remaining movement
            remaining_x -= step_x;
            remaining_y -= step_y;
            
            // Small delay between movements
            random_delay(10, 25);
        }
        
        // Update internal position to match actual cursor position
        self.cursor_x = target_x;
        self.cursor_y = target_y;

        Ok(())
    }

    /// Sends a key press event for the specified key code and action type
    pub fn button(&mut self, button: Button, action: ButtonAction) -> Result<(), Box<dyn std::error::Error>> {
        match action {
            ButtonAction::Down => {
                self.device.write_all(&[button as u8, 0, 0, 0])?;
            }
            ButtonAction::Up => {
                self.device.write_all(&[0, 0, 0, 0])?;
            }
            ButtonAction::Click => {
                self.device.write_all(&[button as u8, 0, 0, 0])?;
                random_delay(50, 150);
                self.device.write_all(&[0, 0, 0, 0])?;
            }
        }
        Ok(())
    }
}