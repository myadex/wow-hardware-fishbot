//! Offline bite-motion diagnostic for a local MP4. No capture or HID devices.
use opencv::{
    core::{self, Mat, Rect},
    imgproc,
    prelude::*,
    videoio,
};
use std::{fs, io::Write};
use wow_hardware_fishbot::VisionResult;

fn changed(diff: &Mat, rect: Rect, threshold: f64) -> VisionResult<i32> {
    let roi = Mat::roi(diff, rect)?;
    let mut mask = Mat::default();
    imgproc::threshold(&roi, &mut mask, threshold, 255.0, imgproc::THRESH_BINARY)?;
    Ok(core::count_non_zero(&mask)?)
}

fn main() -> VisionResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 6 {
        return Err("Usage: video-motion INPUT.mp4 NEW_OUTPUT.csv X Y WIDTH HEIGHT".into());
    }
    let rect = Rect::new(
        args[2].parse()?,
        args[3].parse()?,
        args[4].parse()?,
        args[5].parse()?,
    );
    let mut cap = videoio::VideoCapture::from_file(&args[0], videoio::CAP_ANY)?;
    if !cap.is_opened()? {
        return Err("Video cannot be opened".into());
    }
    let fps = cap.get(videoio::CAP_PROP_FPS)?;
    if fps <= 0.0 {
        return Err("Video FPS unavailable".into());
    }
    let width = cap.get(videoio::CAP_PROP_FRAME_WIDTH)? as i32;
    let height = cap.get(videoio::CAP_PROP_FRAME_HEIGHT)? as i32;
    let pad = 30;
    let outer = Rect::new(
        rect.x - pad,
        rect.y - pad,
        rect.width + 2 * pad,
        rect.height + 2 * pad,
    );
    if rect.width <= 0
        || rect.height <= 0
        || outer.x < 0
        || outer.y < 0
        || outer.x + outer.width > width
        || outer.y + outer.height > height
    {
        return Err("ROI or expanded ROI is outside video".into());
    }
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?;
    writeln!(
        out,
        "frame,seconds,inner50,outer50,ring50,inner20,outer20,ring20,bite_candidate"
    )?;
    let mut previous = Mat::default();
    let mut index = 0;
    loop {
        let mut bgr = Mat::default();
        if !cap.read(&mut bgr)? || bgr.empty() {
            break;
        }
        let mut gray = Mat::default();
        imgproc::cvt_color_def(&bgr, &mut gray, imgproc::COLOR_BGR2GRAY)?;
        if !previous.empty() {
            let mut diff = Mat::default();
            core::absdiff(&gray, &previous, &mut diff)?;
            let inner50 = changed(&diff, rect, 50.0)?;
            let outer50 = changed(&diff, outer, 50.0)?;
            let inner20 = changed(&diff, rect, 20.0)?;
            let outer20 = changed(&diff, outer, 20.0)?;
            let bite_candidate = inner20 as f64 >= rect.area() as f64 * 0.12;
            writeln!(
                out,
                "{index},{:.3},{inner50},{outer50},{},{inner20},{outer20},{},{bite_candidate}",
                index as f64 / fps,
                outer50 - inner50,
                outer20 - inner20
            )?;
        }
        previous = gray;
        index += 1;
    }
    println!("Decoded {index} frames at {fps:.3} fps");
    Ok(())
}
