use opencv::core;
use opencv::prelude::MatTraitConst;
use opencv::{
    Result,
    core::{Mat, Rect, Size, Vector},
    imgcodecs, imgproc,
};
use std::time::{Duration, Instant};
use wow_hardware_fishbot::{
    bite::{measure_splash, monitor_rect as bite_monitor_rect},
    capture::LiveCapture,
    find_bobber_in_frame, load_color_templates, load_templates,
};

use rand::Rng;
use std::error::Error;
use std::path::Path;
use std::thread::sleep;

mod keyboard;
mod mouse;

use keyboard::HidKeyboard;
use mouse::{HidMouse, map_bobber};

/// Introduces a random delay to make the bot behavior less predictable
/// This might helps avoid detection by anti-cheat systems
fn random_delay(min_delay: u64, max_delay: u64) {
    let mut rng = rand::rng();
    let delay_ms = rng.random_range(min_delay..=max_delay);

    sleep(Duration::from_millis(delay_ms))
}

/// Captures a single frame from the video capture device
fn capture_frame(cap: &mut LiveCapture) -> Result<Mat, Box<dyn Error>> {
    cap.fresh_frame()
}

/// During bite monitoring, read each next frame instead of skipping five frames.
fn capture_monitor_frame(cap: &mut LiveCapture) -> Result<Mat, Box<dyn Error>> {
    frame_to_gray(&cap.next_frame()?)
}

fn frame_to_gray(frame: &Mat) -> Result<Mat, Box<dyn Error>> {
    if frame.empty() {
        eprintln!("Error: Captured frame is empty");
        return Err("Empty frame captured".into());
    }

    // Convert to grayscale for image processing
    let mut frame_gray = Mat::default();
    imgproc::cvt_color_def(frame, &mut frame_gray, imgproc::COLOR_BGR2GRAY)?;

    Ok(frame_gray)
}

fn save_debug_frame(frame: &Mat, filename: &str) -> Result<(), Box<dyn Error>> {
    // OpenCV writes either the BGR cast frame or the grayscale monitor frame.
    let params = Vector::new();
    match imgcodecs::imwrite(filename, frame, &params) {
        Ok(_) => println!("Frame saved as '{}'", filename),
        Err(e) => eprintln!("Error saving frame: {}", e),
    }

    Ok(())
}

fn desktop_size_override() -> Result<Option<Size>, Box<dyn Error>> {
    match (
        std::env::var("FISHBOT_SCREEN_WIDTH").ok(),
        std::env::var("FISHBOT_SCREEN_HEIGHT").ok(),
    ) {
        (None, None) => Ok(None),
        (Some(width), Some(height)) => {
            let width: i32 = width.parse()?;
            let height: i32 = height.parse()?;
            if width <= 0 || height <= 0 {
                return Err("Screen dimensions must be positive".into());
            }
            Ok(Some(Size::new(width, height)))
        }
        _ => Err("Set both FISHBOT_SCREEN_WIDTH and FISHBOT_SCREEN_HEIGHT".into()),
    }
}

/// Detects if a fish has "splashed" by comparing two consecutive frames
/// A splash is detected as significant movement/change in the bobber area
fn detect_splash(
    prev_frame: &Mat,
    current_frame: &Mat,
    rect: Rect,
) -> Result<bool, Box<dyn Error>> {
    let measurement = measure_splash(prev_frame, current_frame, rect)?;
    print!(
        "{}/{} ",
        measurement.changed_pixels, measurement.total_pixels
    );
    Ok(measurement.is_candidate())
}

#[cfg(test)]
mod bite_tests {
    use super::*;
    use opencv::prelude::MatTrait;

    #[test]
    fn bite_threshold_scales_with_bobber_area() -> Result<(), Box<dyn Error>> {
        let previous =
            Mat::new_rows_cols_with_default(40, 40, core::CV_8UC1, core::Scalar::all(0.0))?;
        let mut current = previous.try_clone()?;
        let small = Rect::new(0, 0, 10, 10);
        for x in 0..10 {
            *current.at_2d_mut::<u8>(0, x)? = 30;
        }
        *current.at_2d_mut::<u8>(1, 0)? = 30;
        assert!(!detect_splash(&previous, &current, small)?);
        *current.at_2d_mut::<u8>(1, 1)? = 30;
        assert!(detect_splash(&previous, &current, small)?);
        assert!(!detect_splash(
            &previous,
            &current,
            Rect::new(0, 0, 20, 20)
        )?);
        Ok(())
    }

    #[test]
    fn color_crop_uses_inner_bite_window() {
        assert_eq!(
            bite_monitor_rect(Rect::new(414, 236, 32, 32)),
            Rect::new(417, 239, 26, 26)
        );
        assert_eq!(
            bite_monitor_rect(Rect::new(954, 402, 28, 25)),
            Rect::new(954, 402, 28, 25)
        );
        assert_eq!(
            bite_monitor_rect(Rect::new(900, 350, 70, 49)),
            Rect::new(900, 350, 70, 49)
        );
    }
}

/// Continuously monitors for a fish splash within the specified timeout period
fn wait_for_splash(
    cap: &mut LiveCapture,
    lure_location_rect: Rect,
    timeout: Duration,
    mut prev_frame: Mat,
) -> Result<bool, Box<dyn std::error::Error>> {
    let start_time = Instant::now();

    // Keep checking for splashes until timeout
    while Instant::now().duration_since(start_time) < timeout {
        let current_frame = capture_monitor_frame(cap)?;

        // Check if a splash occurred
        if detect_splash(&prev_frame, &current_frame, lure_location_rect)? {
            println!("Splash detected!");
            return Ok(true);
        }

        // Update previous frame for next comparison
        prev_frame = current_frame;

        // Small delay between checks to avoid excessive CPU usage
        sleep(Duration::from_millis(50));
    }

    Ok(false) // Timeout occurred without detecting a splash
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let timeout = Duration::from_secs(29);
    let screen_override = desktop_size_override()?;

    // Initialize the keyboard and relative-mouse HID gadgets.
    let mut keyboard = HidKeyboard::new()?;
    let mut mouse = HidMouse::new()?;

    // Load both video-derived color views and the previous edge templates.
    let templates = load_templates(Path::new("./templates"))?;
    let color_templates = load_color_templates(Path::new("./color-templates"))?;

    // Initialize the video capture device
    let mut cap = LiveCapture::open()?;
    let capture_size = cap.dimensions()?;
    if capture_size.width <= 0 || capture_size.height <= 0 {
        return Err("Capture dimensions are unavailable".into());
    }
    let desktop_size = screen_override.unwrap_or(capture_size);
    println!(
        "Mouse coordinate mapping: capture {}x{} -> desktop {}x{}",
        capture_size.width, capture_size.height, desktop_size.width, desktop_size.height
    );

    // Main fishing loop
    loop {
        // Park the cursor outside the search area before casting. This keeps
        // its image from covering the bobber during bite monitoring.
        mouse.home(desktop_size)?;

        // Cast fishing
        keyboard.tap(0x1f)?;

        // Wait for bobber beeing placed
        sleep(Duration::from_millis(2500));

        // Capture a frame
        let frame = capture_frame(&mut cap)?;

        // Detect bobber on caputre frame
        let Some(detection) = find_bobber_in_frame(&frame, &templates, &color_templates, 0.80)?
        else {
            println!("No reliable bobber match; retrying cast");
            random_delay(1000, 2000);
            continue;
        };

        println!(
            "Bobber template={} score={:.3} x={} y={} width={} height={}",
            detection.template,
            detection.score,
            detection.rect.x,
            detection.rect.y,
            detection.rect.width,
            detection.rect.height
        );
        let lure_location_rect = bite_monitor_rect(detection.rect);

        // DBUG CAPTURE OUTPUT
        let mut debug_frame = frame.try_clone()?;
        imgproc::rectangle(
            &mut debug_frame,
            lure_location_rect,
            core::Scalar::all(255.0),
            2,
            imgproc::LINE_8,
            0,
        )?;
        save_debug_frame(&debug_frame, "captured_frame.jpg")?;

        // Detection/debug encoding can take time. Drain queued frames before
        // starting motion comparisons rather than counting that delay as a bite.
        let baseline = frame_to_gray(&capture_frame(&mut cap)?)?;
        // detect splash
        let splash_detected = wait_for_splash(&mut cap, lure_location_rect, timeout, baseline)?;

        if splash_detected {
            let target = map_bobber(detection.rect, frame.size()?, desktop_size)?;
            println!(
                "Moving to bobber at ({}, {}) and right-clicking",
                target.x, target.y
            );
            mouse.move_from_home(target)?;
            mouse.right_click()?;
        } else {
            println!("Timeout occured while waiting for splash")
        }

        // DBUG CAPTURE OUTPUT
        // Paint rectangle on grayscale debug output.
        let mut debug_frame = capture_frame(&mut cap)?;
        imgproc::rectangle(
            &mut debug_frame,
            lure_location_rect,
            core::Scalar::all(255.0),
            2,
            imgproc::LINE_8,
            0,
        )?;
        save_debug_frame(&debug_frame, "captured_frame.jpg")?;

        // Random delay before repeat
        random_delay(1000, 8000);
    }
}
