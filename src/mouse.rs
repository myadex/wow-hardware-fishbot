use opencv::core::{Point, Rect, Size};
use std::{error::Error, fs::OpenOptions, io::Write, thread::sleep, time::Duration};

const MAX_STEP: i32 = 64;
const REPORT_PAUSE: Duration = Duration::from_millis(2);
const CLICK_HOLD: Duration = Duration::from_millis(80);

/// The configured gadget exposes a four-byte relative mouse report:
/// buttons, signed X, signed Y and wheel.
pub struct HidMouse {
    device: std::fs::File,
    at_home: bool,
}

impl HidMouse {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            device: OpenOptions::new().write(true).open("/dev/hidg1")?,
            at_home: false,
        })
    }

    /// Drive the host pointer into the top-left screen edge before every cast.
    pub fn home(&mut self, desktop: Size) -> Result<(), Box<dyn Error>> {
        if desktop.width <= 0 || desktop.height <= 0 {
            return Err("Desktop dimensions must be positive".into());
        }
        // Twice the larger dimension gives the host ample room to clip at
        // the edge even if one HID count moves less than one desktop pixel.
        let reports = (i64::from(desktop.width.max(desktop.height)) * 2 + 126) / 127 + 2;
        for _ in 0..reports {
            self.device
                .write_all(&[0, (-127i8) as u8, (-127i8) as u8, 0])?;
            sleep(REPORT_PAUSE);
        }
        self.at_home = true;
        Ok(())
    }

    pub fn move_from_home(&mut self, target: Point) -> Result<(), Box<dyn Error>> {
        if !self.at_home {
            return Err("Mouse position is not initialized; call home first".into());
        }
        if target.x < 0 || target.y < 0 {
            return Err("Mouse target must be nonnegative".into());
        }
        for report in movement_reports(target) {
            self.device.write_all(&report)?;
            sleep(REPORT_PAUSE);
        }
        self.at_home = false;
        Ok(())
    }

    pub fn right_click(&mut self) -> Result<(), Box<dyn Error>> {
        write_right_click(&mut self.device, CLICK_HOLD)?;
        Ok(())
    }
}

fn write_right_click(writer: &mut impl Write, hold: Duration) -> std::io::Result<()> {
    writer.write_all(&[2, 0, 0, 0])?;
    sleep(hold);
    writer.write_all(&[0, 0, 0, 0])
}

fn movement_reports(target: Point) -> Vec<[u8; 4]> {
    let mut x = target.x;
    let mut y = target.y;
    let mut reports = Vec::new();
    while x != 0 || y != 0 {
        let step_x = x.min(MAX_STEP);
        let step_y = y.min(MAX_STEP);
        reports.push([0, step_x as u8, step_y as u8, 0]);
        x -= step_x;
        y -= step_y;
    }
    reports
}

/// Scale a captured bobber center to the host desktop. This assumes the HDMI
/// capture shows the complete desktop without cropping or letterboxing.
pub fn map_bobber(rect: Rect, capture: Size, desktop: Size) -> Result<Point, Box<dyn Error>> {
    if capture.width <= 0
        || capture.height <= 0
        || desktop.width <= 0
        || desktop.height <= 0
        || rect.width <= 0
        || rect.height <= 0
        || rect.x < 0
        || rect.y < 0
        || rect.x + rect.width > capture.width
        || rect.y + rect.height > capture.height
    {
        return Err("Invalid bobber rectangle or screen dimensions".into());
    }
    let center_x = rect.x + rect.width / 2;
    let center_y = rect.y + rect.height / 2;
    let x = (i64::from(center_x) * i64::from(desktop.width) / i64::from(capture.width))
        .min(i64::from(desktop.width - 1)) as i32;
    let y = (i64::from(center_y) * i64::from(desktop.height) / i64::from(capture.height))
        .min(i64::from(desktop.height - 1)) as i32;
    Ok(Point::new(x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_reports_are_four_bytes_with_no_buttons_held_during_motion() {
        let reports = movement_reports(Point::new(130, 65));
        assert_eq!(reports, [[0, 64, 64, 0], [0, 64, 1, 0], [0, 2, 0, 0]]);
    }

    #[test]
    fn right_click_presses_and_releases_button_two() {
        let mut output = Vec::new();
        write_right_click(&mut output, Duration::ZERO).unwrap();
        assert_eq!(output, [2, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn maps_capture_center_to_desktop_and_rejects_crop_overflow() {
        let point = map_bobber(
            Rect::new(948, 528, 24, 24),
            Size::new(1920, 1080),
            Size::new(2560, 1440),
        )
        .unwrap();
        assert_eq!(point, Point::new(1280, 720));
        assert!(
            map_bobber(
                Rect::new(1910, 100, 20, 20),
                Size::new(1920, 1080),
                Size::new(2560, 1440)
            )
            .is_err()
        );
    }
}
