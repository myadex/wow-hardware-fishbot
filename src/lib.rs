use opencv::{
    core::{self, Mat, Point, Rect},
    imgcodecs, imgproc,
    prelude::*,
};
use std::{error::Error, path::Path};

pub type VisionResult<T> = Result<T, Box<dyn Error>>;

pub struct Template {
    pub name: String,
    edges: Mat,
}

#[derive(Debug)]
pub struct Detection {
    pub template: String,
    pub score: f64,
    pub rect: Rect,
}

fn edges(gray: &Mat) -> VisionResult<Mat> {
    if gray.empty() || gray.typ() != core::CV_8UC1 {
        return Err("Expected a nonempty 8-bit grayscale image".into());
    }
    let mut result = Mat::default();
    imgproc::canny(gray, &mut result, 50.0, 100.0, 3, false)?;
    Ok(result)
}

pub fn load_templates(directory: &Path) -> VisionResult<Vec<Template>> {
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
        let filename = path.to_str().ok_or("Template path is not UTF-8")?;
        let color = imgcodecs::imread(filename, imgcodecs::IMREAD_COLOR)?;
        if color.empty() {
            return Err(format!("Could not read template: {}", path.display()).into());
        }
        // Use the same conversion as capture and screenshot input. Codec-native
        // grayscale can round differently and change the resulting Canny edges.
        let mut gray = Mat::default();
        imgproc::cvt_color_def(&color, &mut gray, imgproc::COLOR_BGR2GRAY)?;
        let edge_image = edges(&gray).map_err(|e| format!("{}: {e}", path.display()))?;
        let count = core::count_non_zero(&edge_image)?;
        if count == 0 || count == edge_image.rows() * edge_image.cols() {
            return Err(
                format!("Template has no usable edge variation: {}", path.display()).into(),
            );
        }
        templates.push(Template {
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            edges: edge_image,
        });
    }
    if templates.is_empty() {
        return Err("No image templates found".into());
    }
    Ok(templates)
}

/// Searches every fitting template. The score is correlation, not a probability.
pub fn find_bobber(
    gray: &Mat,
    templates: &[Template],
    threshold: f64,
) -> VisionResult<Option<Detection>> {
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return Err("Threshold must be a finite number between 0 and 1".into());
    }
    if templates.is_empty() {
        return Err("No templates supplied".into());
    }
    let frame = edges(gray)?;
    if core::count_non_zero(&frame)? == 0 {
        return Ok(None);
    }
    let mut best: Option<Detection> = None;
    let mut fitting = false;
    for template in templates {
        if template.edges.cols() > frame.cols() || template.edges.rows() > frame.rows() {
            continue;
        }
        fitting = true;
        let mut scores = Mat::default();
        imgproc::match_template(
            &frame,
            &template.edges,
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
        if score.is_finite() && score >= threshold && best.as_ref().is_none_or(|b| score > b.score)
        {
            best = Some(Detection {
                template: template.name.clone(),
                score,
                rect: Rect::new(
                    location.x,
                    location.y,
                    template.edges.cols(),
                    template.edges.rows(),
                ),
            });
        }
    }
    if !fitting {
        return Err("All templates are larger than the input image".into());
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shipped_template_matches_identical_color_input() -> VisionResult<()> {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
        let templates = load_templates(&directory)?;
        let color = imgcodecs::imread(
            directory.join("bobber1.png").to_str().unwrap(),
            imgcodecs::IMREAD_COLOR,
        )?;
        let mut gray = Mat::default();
        imgproc::cvt_color_def(&color, &mut gray, imgproc::COLOR_BGR2GRAY)?;
        let found =
            find_bobber(&gray, &templates, 0.999)?.expect("identical template should match");
        assert_eq!(found.template, "bobber1.png");
        assert_eq!(found.rect, Rect::new(0, 0, gray.cols(), gray.rows()));
        Ok(())
    }
    fn fixture() -> VisionResult<(Mat, Template)> {
        let mut gray =
            Mat::new_rows_cols_with_default(40, 40, core::CV_8UC1, core::Scalar::all(0.0))?;
        imgproc::rectangle(
            &mut gray,
            Rect::new(7, 9, 14, 18),
            core::Scalar::all(255.0),
            -1,
            imgproc::LINE_8,
            0,
        )?;
        let template = Template {
            name: "fixture".into(),
            edges: edges(&gray)?,
        };
        Ok((gray, template))
    }
    #[test]
    fn finds_second_template_and_correct_rectangle() -> VisionResult<()> {
        let (gray, template) = fixture()?;
        let oversized = Template {
            name: "oversized".into(),
            edges: Mat::new_rows_cols_with_default(
                200,
                200,
                core::CV_8UC1,
                core::Scalar::all(0.0),
            )?,
        };
        let mut frame =
            Mat::new_rows_cols_with_default(100, 120, core::CV_8UC1, core::Scalar::all(0.0))?;
        gray.copy_to(&mut Mat::roi_mut(&mut frame, Rect::new(50, 30, 40, 40))?)?;
        let found =
            find_bobber(&frame, &[oversized, template], 0.99)?.expect("fixture should match");
        assert_eq!(found.template, "fixture");
        assert_eq!(found.rect, Rect::new(50, 30, 40, 40));
        Ok(())
    }
    #[test]
    fn rejects_blank_frames_and_invalid_thresholds() -> VisionResult<()> {
        let (gray, template) = fixture()?;
        let templates = [template];
        let blank =
            Mat::new_rows_cols_with_default(100, 100, core::CV_8UC1, core::Scalar::all(0.0))?;
        assert!(find_bobber(&blank, &templates, 0.8)?.is_none());
        assert!(find_bobber(&gray, &templates, f64::NAN).is_err());
        assert!(find_bobber(&gray, &[], 0.8).is_err());
        Ok(())
    }

    #[test]
    fn rejects_unrelated_edges_and_oversized_templates() -> VisionResult<()> {
        let (_, template) = fixture()?;
        let mut different =
            Mat::new_rows_cols_with_default(40, 40, core::CV_8UC1, core::Scalar::all(0.0))?;
        imgproc::line(
            &mut different,
            Point::new(2, 2),
            Point::new(36, 36),
            core::Scalar::all(255.0),
            2,
            imgproc::LINE_8,
            0,
        )?;
        let templates = [template];
        assert!(find_bobber(&different, &templates, 0.99)?.is_none());
        let small = Mat::roi(&different, Rect::new(0, 0, 20, 20))?.try_clone()?;
        assert!(find_bobber(&small, &templates, 0.80).is_err());
        Ok(())
    }
}
