use opencv::core;
use opencv::prelude::{MatTraitConst, VideoCaptureTrait, VideoCaptureTraitConst};
use opencv::videoio;
use opencv::videoio::{CAP_V4L2, VideoCapture};
use opencv::{
    Result,
    core::{Mat, Rect, Vector},
    imgcodecs, imgproc,
};
use std::time::{Duration, Instant};
use wow_hardware_fishbot::{find_bobber, load_templates};

use rand::Rng;
use std::error::Error;
use std::path::Path;
use std::thread::sleep;

mod keyboard;

use keyboard::HidKeyboard;

/// Introduces a random delay to make the bot behavior less predictable
/// This might helps avoid detection by anti-cheat systems
fn random_delay(min_delay: u64, max_delay: u64) {
    let mut rng = rand::rng();
    let delay_ms = rng.random_range(min_delay..=max_delay);

    sleep(Duration::from_millis(delay_ms))
}

/// Initializes and configures the video capture device
fn capture_init() -> Result<VideoCapture, Box<dyn Error>> {
    println!("Opening video device /dev/video0...");

    // Open the video capture device
    let mut cap = VideoCapture::new(0, CAP_V4L2)?;

    // Check if the camera is opened successfully
    if !cap.is_opened()? {
        eprintln!("Error: Could not open video device /dev/video0");
        return Err("Failed to open video device".into());
    }

    println!("Video device opened successfully!");

    // Set video format properties exactly like your Python code
    cap.set(videoio::CAP_PROP_FRAME_WIDTH, 1920.0)?;
    cap.set(videoio::CAP_PROP_FRAME_HEIGHT, 1080.0)?;

    // Set FOURCC to RGB3 format
    let fourcc_rgb3 = (82u32) | (71u32 << 8) | (66u32 << 16) | (51u32 << 24);
    cap.set(videoio::CAP_PROP_FOURCC, fourcc_rgb3 as f64)?;

    // Set buffer size (equivalent to --stream-mmap=4)
    cap.set(videoio::CAP_PROP_BUFFERSIZE, 1.0)?;

    // Print actual resolution to verify settings
    let width = cap.get(videoio::CAP_PROP_FRAME_WIDTH)?;
    let height = cap.get(videoio::CAP_PROP_FRAME_HEIGHT)?;
    println!("Resolution: {}x{}", width as i32, height as i32);

    Ok(cap)
}

/// Captures a single frame from the video capture device
fn capture_frame(cap: &mut VideoCapture) -> Result<Mat, Box<dyn Error>> {
    // Create matrix to hold the frame
    let mut frame = Mat::default();

    // This is a little hack but we have to some how grab a few frames
    // before we decode it. Otherwise we might get an old frame.
    for _ in 0..5 {
        cap.grab()?;
    }

    // Now decode the latest frame we grabbed
    cap.retrieve(&mut frame, 0)?;

    frame_to_gray(&frame)
}

/// During bite monitoring, read each next frame instead of skipping five frames.
fn capture_monitor_frame(cap: &mut VideoCapture) -> Result<Mat, Box<dyn Error>> {
    let mut frame = Mat::default();
    cap.read(&mut frame)?;
    frame_to_gray(&frame)
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
    // Capture frames are already grayscale; OpenCV can write them directly.
    let params = Vector::new();
    match imgcodecs::imwrite(filename, frame, &params) {
        Ok(_) => println!("Frame saved as '{}'", filename),
        Err(e) => eprintln!("Error saving frame: {}", e),
    }

    Ok(())
}

/// Detects if a fish has "splashed" by comparing two consecutive frames
/// A splash is detected as significant movement/change in the bobber area
fn detect_splash(
    prev_frame: &Mat,
    current_frame: &Mat,
    rect: Rect,
) -> Result<bool, Box<dyn Error>> {
    // Calculate the absolute difference between frames
    let mut diff_frame = Mat::default();
    core::absdiff(prev_frame, current_frame, &mut diff_frame)?;

    // Extract the region of interest (ROI) around the bobber
    let roi = Mat::roi(&diff_frame, rect)?;

    // Apply threshold to convert differences to binary (black/white)
    let mut thresh_frame = Mat::default();
    imgproc::threshold(&roi, &mut thresh_frame, 20.0, 255.0, imgproc::THRESH_BINARY)?;

    // Count non-zero pixels (white pixels indicating movement)
    let non_zero_count = core::count_non_zero(&thresh_frame)?;

    // If enough pixels changed, consider it a splash
    let splash_detected = (non_zero_count as f64) >= rect.area() as f64 * 0.12;

    print!("{non_zero_count}/{} ", rect.area());

    Ok(splash_detected)
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
}

/// Continuously monitors for a fish splash within the specified timeout period
fn wait_for_splash(
    cap: &mut VideoCapture,
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
    let bite_key_name = std::env::var("FISHBOT_BITE_KEY").unwrap_or_else(|_| "F8".to_owned());
    let bite_key = keyboard::parse_key_name(&bite_key_name)?;

    // Initialize the keyboard HID gadget.
    let mut keyboard = HidKeyboard::new()?;

    // Load the templates
    let templates = load_templates(Path::new("./templates"))?;

    // Initialize the video capture device
    let mut cap = capture_init()?;

    // Main fishing loop
    loop {
        // Cast fishing
        keyboard.tap(0x1f)?;

        // Wait for bobber beeing placed
        sleep(Duration::from_millis(2500));

        // Capture a frame
        let frame = capture_frame(&mut cap)?;

        // Detect bobber on caputre frame
        let Some(detection) = find_bobber(&frame, &templates, 0.80)? else {
            println!("No reliable bobber match; retrying cast");
            random_delay(1000, 2000);
            continue;
        };

        // Create rectangle surrounding bobber
        let lure_location_rect = detection.rect;

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

        // detect splash
        let splash_detected = wait_for_splash(&mut cap, lure_location_rect, timeout, frame)?;

        if splash_detected {
            keyboard.tap(bite_key)?;
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
