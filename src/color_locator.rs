use crate::{Detection, VisionResult};
use opencv::{
    core::{self, Mat, Point, Rect, Size},
    imgcodecs, imgproc,
    prelude::*,
};
use std::path::Path;

pub struct ColorTemplate {
    name: String,
    bgr: Mat,
}

pub fn load_color_templates(directory: &Path) -> VisionResult<Vec<ColorTemplate>> {
    let mut paths = std::fs::read_dir(directory)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    let mut templates = Vec::new();
    for path in paths {
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !path.is_file() || !matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "bmp") {
            continue;
        }
        let bgr = imgcodecs::imread(
            path.to_str().ok_or("Template path is not UTF-8")?,
            imgcodecs::IMREAD_COLOR,
        )?;
        if bgr.empty() || bgr.cols() < 2 || bgr.rows() < 2 {
            return Err(format!("Invalid color template: {}", path.display()).into());
        }
        templates.push(ColorTemplate {
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            bgr,
        });
    }
    if templates.is_empty() {
        return Err("No color templates found".into());
    }
    Ok(templates)
}

fn smooth(image: &Mat) -> VisionResult<Mat> {
    let mut result = Mat::default();
    imgproc::gaussian_blur_def(image, &mut result, Size::new(5, 5), 1.0)?;
    Ok(result)
}

/// Searches the central water area with resized, smoothed color crops.
/// The score is normalized correlation, not a probability.
pub fn find_color_bobber(
    bgr: &Mat,
    templates: &[ColorTemplate],
    threshold: f64,
) -> VisionResult<Option<Detection>> {
    if bgr.empty() || bgr.typ() != core::CV_8UC3 {
        return Err("Expected a nonempty 8-bit BGR image".into());
    }
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return Err("Threshold must be a finite number between 0 and 1".into());
    }
    if templates.is_empty() {
        return Err("No color templates supplied".into());
    }
    // The normalized region excludes most screen UI while containing all
    // confirmed bobber positions in the local screenshots and recordings.
    let x0 = (bgr.cols() as f64 * 0.15).round() as i32;
    let y0 = (bgr.rows() as f64 * 0.12).round() as i32;
    let x1 = (bgr.cols() as f64 * 0.87).round() as i32;
    let y1 = (bgr.rows() as f64 * 0.64).round() as i32;
    let region = Rect::new(x0, y0, x1 - x0, y1 - y0);
    if region.width < 2 || region.height < 2 {
        return Err("Input image is too small for color search".into());
    }
    let smooth_frame = smooth(bgr)?;
    let search = Mat::roi(&smooth_frame, region)?;
    let mut best: Option<Detection> = None;
    let mut fitting = false;
    for template in templates {
        for scale in [0.8, 1.0, 1.2] {
            let mut resized = Mat::default();
            imgproc::resize(
                &template.bgr,
                &mut resized,
                Size::default(),
                scale,
                scale,
                imgproc::INTER_LINEAR,
            )?;
            if resized.cols() > search.cols() || resized.rows() > search.rows() {
                continue;
            }
            fitting = true;
            let smoothed_template = smooth(&resized)?;
            let mut scores = Mat::default();
            imgproc::match_template(
                &search,
                &smoothed_template,
                &mut scores,
                imgproc::TM_CCOEFF_NORMED,
                &Mat::default(),
            )?;
            let mut score = 0.0;
            let mut location = Point::default();
            core::min_max_loc(
                &scores,
                None,
                Some(&mut score),
                None,
                Some(&mut location),
                &Mat::default(),
            )?;
            if score.is_finite()
                && score >= threshold
                && best.as_ref().is_none_or(|b| score > b.score)
            {
                best = Some(Detection {
                    template: format!("{}@{scale}", template.name),
                    score,
                    rect: Rect::new(
                        location.x + region.x,
                        location.y + region.y,
                        smoothed_template.cols(),
                        smoothed_template.rows(),
                    ),
                });
            }
        }
    }
    if !fitting {
        return Err("All color templates are larger than the search area".into());
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_color_crop_is_found_and_blank_frame_is_rejected() -> VisionResult<()> {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("color-templates");
        let templates = load_color_templates(&directory)?;
        let crop = templates
            .iter()
            .find(|t| t.name == "video1-second-bobber.png")
            .expect("shipped video crop");
        let mut frame = Mat::new_rows_cols_with_default(
            150,
            200,
            core::CV_8UC3,
            core::Scalar::new(20.0, 40.0, 20.0, 0.0),
        )?;
        assert!(find_color_bobber(&frame, &templates, 0.78)?.is_none());
        crop.bgr.copy_to(&mut Mat::roi_mut(
            &mut frame,
            Rect::new(70, 40, crop.bgr.cols(), crop.bgr.rows()),
        )?)?;
        let found = find_color_bobber(&frame, &templates, 0.78)?.expect("placed bobber");
        assert!(found.rect.contains(Point::new(86, 56)));
        assert!(find_color_bobber(&frame, &templates, f64::NAN).is_err());
        assert!(crate::find_bobber_in_frame(&frame, &[], &templates, f64::NAN).is_err());
        Ok(())
    }
}
