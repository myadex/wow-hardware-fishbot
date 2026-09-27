//! Offline color-template probe. No capture or HID devices.
use opencv::{
    core::{self, Mat, Point, Rect, Size},
    imgcodecs, imgproc,
    prelude::*,
};
use std::{fs, path::Path};
use wow_hardware_fishbot::VisionResult;

fn smooth(image: &Mat) -> VisionResult<Mat> {
    let mut result = Mat::default();
    imgproc::gaussian_blur_def(image, &mut result, Size::new(5, 5), 1.0)?;
    Ok(result)
}

fn main() -> VisionResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: color-image-test INPUT_IMAGE TEMPLATE_DIRECTORY".into());
    }
    let image = imgcodecs::imread(&args[0], imgcodecs::IMREAD_COLOR)?;
    if image.empty() {
        return Err("Input image unreadable".into());
    }
    let smooth_frame = smooth(&image)?;
    // Keep the search on the water around the character; screen-edge UI can
    // correlate strongly with dark bobber crops. Fractions also fit local MP4s.
    let x0 = (image.cols() as f64 * 0.15).round() as i32;
    let y0 = (image.rows() as f64 * 0.12).round() as i32;
    let x1 = (image.cols() as f64 * 0.87).round() as i32;
    let y1 = (image.rows() as f64 * 0.64).round() as i32;
    let region = Rect::new(x0, y0, x1 - x0, y1 - y0);
    let search = Mat::roi(&smooth_frame, region)?;
    let mut best = (-2.0, String::new(), Point::default(), Size::default());
    for entry in fs::read_dir(Path::new(&args[1]))? {
        let path = entry?.path();
        if !path.is_file()
            || !matches!(
                path.extension()
                    .and_then(|x| x.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("png" | "jpg" | "jpeg")
            )
        {
            continue;
        }
        let source = imgcodecs::imread(
            path.to_str().ok_or("Invalid template path")?,
            imgcodecs::IMREAD_COLOR,
        )?;
        if source.empty() {
            return Err("Unreadable template".into());
        }
        for scale in [0.8, 1.0, 1.2] {
            let mut resized = Mat::default();
            imgproc::resize(
                &source,
                &mut resized,
                Size::default(),
                scale,
                scale,
                imgproc::INTER_LINEAR,
            )?;
            if resized.cols() > search.cols() || resized.rows() > search.rows() {
                continue;
            }
            let template = smooth(&resized)?;
            let mut scores = Mat::default();
            imgproc::match_template(
                &search,
                &template,
                &mut scores,
                imgproc::TM_CCOEFF_NORMED,
                &Mat::default(),
            )?;
            let mut score = 0.0;
            let mut point = Point::default();
            core::min_max_loc(
                &scores,
                None,
                Some(&mut score),
                None,
                Some(&mut point),
                &Mat::default(),
            )?;
            if score.is_finite() && score > best.0 {
                best = (
                    score,
                    format!("{}@{scale}", path.file_name().unwrap().to_string_lossy()),
                    Point::new(point.x + region.x, point.y + region.y),
                    template.size()?,
                );
            }
        }
    }
    println!(
        "score={:.6} template={} x={} y={} width={} height={}",
        best.0, best.1, best.2.x, best.2.y, best.3.width, best.3.height
    );
    Ok(())
}
