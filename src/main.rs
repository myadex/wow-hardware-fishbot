use std::time::{Duration, Instant};
use opencv::{core::{Vector, Rect, Mat}, imgproc, imgcodecs, Result};
use opencv::videoio;
use opencv::videoio::{VideoCapture, CAP_V4L2};
use wow_hardware_fishbot::{load_templates, find_bobber};
use opencv::core;
use opencv::prelude::{MatTraitConst, VideoCaptureTrait, VideoCaptureTraitConst};


use std::path::Path;
use std::error::Error;
use std::thread::sleep;
use rand::Rng;

mod keyboard;
mod mouse;

use keyboard::{HidKeyboard, KeyAction};
use mouse::{HidMouse, ButtonAction, Button};

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

    // Check if frame is empty
    if frame.empty() {
        eprintln!("Error: Captured frame is empty");
        return Err("Empty frame captured".into());
    }

    // Convert to grayscale for image processing
    let mut frame_gray = Mat::default();
    imgproc::cvt_color_def(&frame, &mut frame_gray, imgproc::COLOR_BGR2GRAY)?;
    
    Ok(frame_gray)
}

fn save_debug_frame(
    frame: &Mat,
    filename: &str,
) -> Result<(), Box<dyn Error>> {

    // Convert RGB to BGR for proper JPEG saving
    let mut bgr_frame = Mat::default();
    imgproc::cvt_color_def(&frame, &mut bgr_frame, imgproc::COLOR_RGB2BGR)?;
    
    // Save the BGR frame as an image
    let params = Vector::new();
    match imgcodecs::imwrite(filename, &bgr_frame, &params) {
        Ok(_) => println!("Frame saved as '{}'", filename),
        Err(e) => eprintln!("Error saving frame: {}", e),
    }
    
    Ok(())
}

fn capture_cleanup(mut cap: VideoCapture) -> Result<(), Box<dyn Error>> {
    cap.release()?;
    println!("Video device released.");
    Ok(())
}

/// Detects if a fish has "splashed" by comparing two consecutive frames
/// A splash is detected as significant movement/change in the bobber area
fn detect_splash(prev_frame: &Mat, current_frame: &Mat, rect: Rect) -> Result<bool, Box<dyn Error>> {
    // Calculate the absolute difference between frames
    let mut diff_frame = Mat::default();
    core::absdiff(prev_frame, current_frame, &mut diff_frame)?;

    // Extract the region of interest (ROI) around the bobber
    let roi = Mat::roi(&diff_frame, rect)?;

    // Apply threshold to convert differences to binary (black/white)
    let mut thresh_frame = Mat::default();
    imgproc::threshold(&roi, &mut thresh_frame, 50.0, 255.0, imgproc::THRESH_BINARY)?;

    // Count non-zero pixels (white pixels indicating movement)
    let non_zero_count = core::count_non_zero(&thresh_frame)?;

    // If enough pixels changed, consider it a splash
    let splash_detected = non_zero_count > 250; // Todo: Adjust this threshold based on experimentation

    print!("{:?} ", non_zero_count);

    Ok(splash_detected)
}

/// Continuously monitors for a fish splash within the specified timeout period
fn wait_for_splash(
    cap: &mut VideoCapture,
    lure_location_rect: Rect, 
    timeout: Duration
) -> Result<bool, Box<dyn std::error::Error>> {
    // Capture initial frame for comparison
    let mut prev_frame = capture_frame(cap)?;
    let start_time = Instant::now();

    // Keep checking for splashes until timeout
    while Instant::now().duration_since(start_time) < timeout {
        let current_frame = capture_frame(cap)?;

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

    // Initalise HID gadgets
    let mut keyboard = HidKeyboard::new()?;
    let mut mouse = HidMouse::new()?;

    // Place cursor to the top left so we initally know it position
    mouse.cursor_home()?;
  
    // Load the templates
    let templates = load_templates(Path::new("./templates"))?;

    // Initialize the video capture device
    let mut cap = capture_init()?;
    
    // Main fishing loop
    loop {
        // Cast fishing
        keyboard.key(0x1f, KeyAction::Tap).unwrap();

        // Wait for bobber beeing placed
        sleep(Duration::from_millis(2500));

        // Capture a frame
        let frame = capture_frame(&mut cap)?;

        // Detect bobber on caputre frame
        let Some(detection) = find_bobber(&frame, &templates, 0.80)? else {
            println!("No reliable bobber match; skipping mouse interaction");
            random_delay(1000, 2000);
            continue;
        };

        // Create rectangle surrounding bobber
        let lure_location_rect = detection.rect;

        // Move mouse to bobber location
        let bobber_x = lure_location_rect.x + lure_location_rect.width / 2;
        let bobber_y = lure_location_rect.y + lure_location_rect.height / 2;
        //mouse.cursor_home()?;
        mouse.cursor_move(bobber_x, bobber_y)?;
        //mouse.cursor_move(200, 200)?;
        
        // wait so the splash detector is not disturbed by the moving cursor
        sleep(Duration::from_millis(600));

        // DBUG CAPTURE OUTPUT
        let mut debug_frame = capture_frame(&mut cap)?;
        imgproc::rectangle(
            &mut debug_frame,
            lure_location_rect,
            core::Scalar::new(255.0, 0.0, 255.0, 0.0),
            2,
            imgproc::LINE_8,
            0,
        )?;
        save_debug_frame(&debug_frame, "captured_frame.jpg")?;

        // detect splash
        let splash_detected = wait_for_splash(&mut cap, lure_location_rect, timeout)?;
        
        if splash_detected {
            mouse.button(Button::Right, ButtonAction::Click)?;
        } else {
            println!("Timeout occured while waiting for splash")
        }

        // DBUG CAPTURE OUTPUT
        // Paint rectangle on debug output (magenta color)
        let mut debug_frame = capture_frame(&mut cap)?;
        imgproc::rectangle(
            &mut debug_frame,
            lure_location_rect,
            core::Scalar::new(255.0, 0.0, 255.0, 0.0),
            2,
            imgproc::LINE_8,
            0,
        )?;
        save_debug_frame(&debug_frame, "captured_frame.jpg")?;

        // Random delay before repeat
        random_delay(1000, 8000);
    }
    // Clean up the video capture device
    capture_cleanup(cap)?;
  
    Ok(())
}
