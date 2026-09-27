//! Extracts local video frames for offline visual review. No HID devices.
use opencv::{
    core::{Mat, Vector},
    imgcodecs,
    prelude::*,
    videoio,
};
use std::{fs, path::Path};
use wow_hardware_fishbot::VisionResult;

fn main() -> VisionResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(3..=5).contains(&args.len()) {
        return Err("Usage: video-sample INPUT.mp4 OUTPUT_DIRECTORY INTERVAL_SECONDS [START_SECONDS] [END_SECONDS]".into());
    }
    let interval: f64 = args[2].parse()?;
    if !interval.is_finite() || interval <= 0.0 {
        return Err("Interval must be positive and finite".into());
    }
    let mut cap = videoio::VideoCapture::from_file(&args[0], videoio::CAP_ANY)?;
    if !cap.is_opened()? {
        return Err(format!("Could not open video: {}", args[0]).into());
    }
    let fps = cap.get(videoio::CAP_PROP_FPS)?;
    let frames = cap.get(videoio::CAP_PROP_FRAME_COUNT)?;
    if fps <= 0.0 || frames <= 0.0 {
        return Err("Video reports no frames or frame rate".into());
    }
    let duration = frames / fps;
    println!(
        "fps={fps:.3} frames={frames:.0} duration={duration:.3}s width={} height={}",
        cap.get(videoio::CAP_PROP_FRAME_WIDTH)?,
        cap.get(videoio::CAP_PROP_FRAME_HEIGHT)?
    );
    let output = Path::new(&args[1]);
    fs::create_dir_all(output)?;
    let mut time: f64 = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(0.0);
    let end: f64 = args
        .get(4)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(duration);
    if !time.is_finite()
        || !end.is_finite()
        || time < 0.0
        || end <= time
        || end > duration + interval
    {
        return Err("Invalid sampling time range".into());
    }
    let mut count = 0;
    while time < duration && time < end {
        cap.set(videoio::CAP_PROP_POS_MSEC, time * 1000.0)?;
        let mut frame = Mat::default();
        if !cap.read(&mut frame)? || frame.empty() {
            break;
        }
        let path = output.join(format!("frame-{count:04}.jpg"));
        if !imgcodecs::imwrite(
            path.to_str().ok_or("Invalid output path")?,
            &frame,
            &Vector::new(),
        )? {
            return Err("Frame write failed".into());
        }
        println!("{time:.3}s {}", path.display());
        count += 1;
        time += interval;
    }
    Ok(())
}
