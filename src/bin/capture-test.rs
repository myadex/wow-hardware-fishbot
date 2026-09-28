//! Save a live frame and a detection review without opening any HID device.
use opencv::{
    core::{Mat, Scalar, Vector},
    imgcodecs, imgproc,
    prelude::*,
};
use std::{io::Write, path::Path};
use wow_hardware_fishbot::{
    VisionResult, capture::LiveCapture, find_bobber_in_frame, load_color_templates, load_templates,
};

fn save_new(path: &Path, frame: &Mat) -> VisionResult<()> {
    let mut bytes = Vector::<u8>::new();
    if !imgcodecs::imencode(".png", frame, &mut bytes, &Vector::new())? {
        return Err("Could not encode captured frame".into());
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes.as_slice())?;
    Ok(())
}

fn main() -> VisionResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h") {
        println!(
            "capture-test RAW.png REVIEW.png\nRun from the repository root after HDMI setup. FISHBOT_SWAP_RB=1 enables the C790 color correction. No mouse or keyboard input. Exit 0: match, 2: no match, 1: error."
        );
        return Ok(());
    }
    if args.len() != 2 {
        return Err("Usage: capture-test RAW.png REVIEW.png".into());
    }
    for name in &args {
        let path = Path::new(name);
        if path.exists() || path.extension().and_then(|e| e.to_str()) != Some("png") {
            return Err("Choose two new .png output filenames".into());
        }
    }
    if args[0] == args[1] {
        return Err("Raw and review output paths must be different".into());
    }
    let edge_templates = load_templates(Path::new("templates"))?;
    let color_templates = load_color_templates(Path::new("color-templates"))?;
    let mut capture = LiveCapture::open()?;
    let frame = capture.fresh_frame()?;
    save_new(Path::new(&args[0]), &frame)?;
    let found = find_bobber_in_frame(&frame, &edge_templates, &color_templates, 0.80)?;
    let mut review = frame.try_clone()?;
    if let Some(detection) = &found {
        println!(
            "MATCH template={} score={:.4} x={} y={} width={} height={}",
            detection.template,
            detection.score,
            detection.rect.x,
            detection.rect.y,
            detection.rect.width,
            detection.rect.height
        );
        imgproc::rectangle(
            &mut review,
            detection.rect,
            Scalar::new(0.0, 255.0, 0.0, 0.0),
            2,
            imgproc::LINE_8,
            0,
        )?;
    } else {
        println!("NO MATCH; inspect the raw frame before changing thresholds");
    }
    save_new(Path::new(&args[1]), &review)?;
    println!("Raw: {}\nReview: {}", args[0], args[1]);
    if found.is_none() {
        std::process::exit(2);
    }
    Ok(())
}
