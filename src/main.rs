use std::{env::current_dir, time::{Duration, Instant}};
use opencv::{core::{Vector, Point, Rect, Mat, CV_8UC4}, imgproc, imgcodecs, Result};
use opencv::videoio;
use opencv::videoio::{VideoCapture, CAP_V4L2};
use opencv::core::min_max_loc;
use opencv::core;
use opencv::prelude::{MatTraitConst, VideoCaptureTrait, VideoCaptureTraitConst};


use std::fs;
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
   
    // Capture a single frame
    //println!("Capturing frame...");

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
   
    //println!("Frame captured successfully! Size: {}x{}",
    //         frame.cols(), frame.rows());

    // Convert to grayscale for image processing
    let mut frame_gray = Mat::default();
    imgproc::cvt_color(&frame, &mut frame_gray, imgproc::COLOR_BGR2GRAY, 0)?;
    
    Ok(frame_gray)
}

fn capture_and_save_debug_frame(
    cap: &mut VideoCapture,
    rect: Rect,
    filename: &str,
) ->Result<Mat, Box<dyn Error>> {
    // Capture frame
    let mut frame = capture_frame(cap)?;
    
    // Paint rectangle on debug output (magenta color)
    imgproc::rectangle(
        &mut frame,
        rect,
        core::Scalar::new(255.0, 0.0, 255.0, 0.0),
        2,
        imgproc::LINE_8,
        0,
    )?;
    
    // Convert RGB to BGR for proper JPEG saving
    let mut bgr_frame = Mat::default();
    imgproc::cvt_color(&frame, &mut bgr_frame, imgproc::COLOR_RGB2BGR, 0)?;
    
    // Save the BGR frame as an image
    let params = Vector::new();
    match imgcodecs::imwrite(filename, &bgr_frame, &params) {
        Ok(_) => println!("Frame saved as '{}'", filename),
        Err(e) => eprintln!("Error saving frame: {}", e),
    }
    
    Ok(frame)
}

fn capture_cleanup(mut cap: VideoCapture) -> Result<(), Box<dyn Error>> {
    cap.release()?;
    println!("Video device released.");
    Ok(())
}

/// Loads fishing bobber template images from the ./template directory
/// These templates are used for computer vision to detect the fishing bobber on screen
fn load_templates() -> Result<Vec<Mat>, Box<dyn Error>> {
    let mut templates: Vec<Mat> = Vec::new();

    let path = Path::new("./templates");
    for entry in fs::read_dir(path)?{
            let entry = entry?;
            println!("{:?}", entry.path().display());

            // Load the template image in color
            let template = imgcodecs::imread(&entry.path().display().to_string(), 1 as i32)?;

            // Convert to grayscale for better template matching
            let mut template_gray = Mat::default();
            imgproc::cvt_color(&template, &mut template_gray, imgproc::COLOR_BGR2GRAY, 0)?;

            // Apply Canny edge detection to focus on edges rather than colors
            // This makes template matching more robust to lighting changes
            let mut template_canny = Mat::default();
            imgproc::canny(&template_gray, &mut template_canny, 50 as f64, 100 as f64, 3, false)?;
            templates.push(template_canny);
    }
    Ok(templates)
}

/// Finds the fishing bobber in the current frame using template matching
fn find_bobber(frame_gray: &Mat, templates: &Vec<Mat>) -> Result<Point, Box<dyn Error>> {
    // Apply Canny edge detection to the current frame (same as on templates)
    let mut frame_canny = Mat::default();
    imgproc::canny(&frame_gray, &mut frame_canny, 50 as f64, 100 as f64, 3, false)?;
 
    // Perform template matching to find the bobber location
    let mut frame_lure_location = Mat::default();
    imgproc::match_template(&frame_canny, &templates[0], &mut frame_lure_location, imgproc::TM_CCOEFF_NORMED, &Mat::default())?;

    // Find the location with the highest match confidence
    let mut max_val = 0.0;
    let mut lure_location = Point::new(0, 0);
    min_max_loc(&frame_lure_location, None, Some(&mut max_val), None, Some(&mut lure_location), &Mat::default())?;
    println!("Bobber dectetion: max_val = {:?}  lure_location = {:?} ", max_val, lure_location);
    Ok(lure_location)
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

    let mut keyboard = HidKeyboard::new()?;
    let mut mouse = HidMouse::new()?;
    mouse.cursor_home()?;
    sleep(Duration::from_millis(100));
  
    // Load the templates
    let templates = load_templates()?;

    // Initialize the video capture device
    let mut cap = capture_init()?;
    
    // Main fishing loop
    loop {
        // Cast fishing
        keyboard.key(0x1f, KeyAction::Tap).unwrap();

        // Wait for bobber beeing placed
        sleep(Duration::from_millis(2000));

        // Capture a frame
        let mut frame = capture_frame(&mut cap)?;

        // Detect bobber on caputre frame
        let lure_location = find_bobber(&frame, &templates)?;

        // Create rectangle surrounding bobber
        let lure_location_rect = Rect::new(lure_location.x, lure_location.y, templates[0].cols(), templates[0].rows());

        // Move mouse to bobber location
        let bobber_x = lure_location_rect.x + lure_location_rect.width / 2;
        let bobber_y = lure_location_rect.y + lure_location_rect.height / 2;
        //mouse.cursor_home()?;
        mouse.cursor_move(bobber_x, bobber_y)?;
        //mouse.cursor_move(200, 200)?;
        
        // wait so the splash detector is not disturbed by the moving cursor
        sleep(Duration::from_millis(600));

        // DBUG CAPTURE OUTPUT
        let _frame = capture_and_save_debug_frame(&mut cap, lure_location_rect, "captured_frame.jpg")?;

        // detect splash
        let splash_detected = wait_for_splash(&mut cap, lure_location_rect, timeout)?;
        
        if splash_detected {
            mouse.button(Button::Right, ButtonAction::Click)?;
        } else {
            println!("Timeout occured while waiting for splash")
        }

        // DBUG CAPTURE OUTPUT
        let _frame = capture_and_save_debug_frame(&mut cap, lure_location_rect, "captured_frame.jpg")?;

        random_delay(1000, 8000);
    }
        
    // Clean up the video capture device
    capture_cleanup(cap)?;
  
    Ok(())
}