use std::fs::OpenOptions;
use std::io::Write;

pub fn press_key(key_code: u8) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = OpenOptions::new().write(true).open("/dev/hidg0")?;
    
    // Press key: [0, 0, key_code, 0, 0, 0, 0, 0]
    file.write_all(&[0, 0, key_code, 0, 0, 0, 0, 0])?;
    
    // Release key: all zeros
    std::thread::sleep(std::time::Duration::from_millis(100));
    file.write_all(&[0, 0, 0, 0, 0, 0, 0, 0])?;
    
    Ok(())
}