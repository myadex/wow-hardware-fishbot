//! Offline experiment. Does not change production detection or use HID devices.
use opencv::{
    core::{self, Mat, Point, Rect, Size},
    imgcodecs, imgproc,
    prelude::*,
};
use std::{fs, io::Write, path::Path};
use wow_hardware_fishbot::VisionResult;
use wow_hardware_fishbot::colors::{ColorMap, TriColorConfig};

/// Soft spatial weighting, not a learned foreground segmentation.
fn center_mask(size: Size) -> VisionResult<Mat> {
    let mut mask = Mat::new_rows_cols_with_default(
        size.height,
        size.width,
        core::CV_32FC1,
        core::Scalar::all(0.0),
    )?;
    for y in 0..size.height {
        for x in 0..size.width {
            let dx = (x as f64 - (size.width - 1) as f64 / 2.0) / (size.width as f64 * 0.28);
            let dy = (y as f64 - (size.height - 1) as f64 / 2.0) / (size.height as f64 * 0.28);
            *mask.at_2d_mut::<f32>(y, x)? = (-0.5 * (dx * dx + dy * dy)).exp() as f32;
        }
    }
    Ok(mask)
}

fn sanitize_scores(scores: &mut Mat) -> VisionResult<()> {
    // Masked normalized correlation can divide by zero on constant patches.
    for value in scores.data_typed_mut::<f32>()? {
        if !value.is_finite() {
            *value = -2.0;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_constant_feature_patch_cannot_be_a_perfect_match() -> VisionResult<()> {
        let mut image =
            Mat::new_rows_cols_with_default(12, 12, core::CV_32FC1, core::Scalar::all(0.1))?;
        *image.at_2d_mut::<f32>(2, 2)? = 0.10001;
        *image.at_2d_mut::<f32>(9, 9)? = 0.9;
        let mut scores =
            Mat::new_rows_cols_with_default(9, 9, core::CV_32FC1, core::Scalar::all(1.0))?;
        reject_flat_patches(&mut scores, &image, Size::new(4, 4))?;
        assert_eq!(*scores.at_2d::<f32>(0, 0)?, -2.0);
        assert_eq!(*scores.at_2d::<f32>(8, 8)?, 1.0);
        Ok(())
    }

    #[test]
    fn undefined_masked_correlations_do_not_hide_finite_candidates() -> VisionResult<()> {
        let mut scores =
            Mat::new_rows_cols_with_default(1, 4, core::CV_32FC1, core::Scalar::all(0.0))?;
        scores.data_typed_mut::<f32>()?.copy_from_slice(&[
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            0.8,
        ]);
        sanitize_scores(&mut scores)?;
        let mut maximum = 0.0;
        let mut point = Point::default();
        core::min_max_loc(
            &scores,
            None,
            Some(&mut maximum),
            None,
            Some(&mut point),
            &Mat::default(),
        )?;
        assert_eq!(point, Point::new(3, 0));
        assert!((maximum - 0.8).abs() < 1e-6);
        Ok(())
    }
}

fn reject_flat_patches(
    scores: &mut Mat,
    search: &impl core::ToInputArray,
    size: Size,
) -> VisionResult<()> {
    let mut sum = Mat::default();
    let mut squared = Mat::default();
    imgproc::integral2(search, &mut sum, &mut squared, core::CV_64F, core::CV_64F)?;
    let stride = sum.cols() as usize;
    let sums = sum.data_typed::<f64>()?;
    let squares = squared.data_typed::<f64>()?;
    let width = scores.cols() as usize;
    let height = scores.rows() as usize;
    let values = scores.data_typed_mut::<f32>()?;
    let area = size.area() as f64;
    for y in 0..height {
        for x in 0..width {
            let a = y * stride + x;
            let b = a + size.width as usize;
            let c = a + size.height as usize * stride;
            let d = c + size.width as usize;
            let mean = (sums[d] - sums[b] - sums[c] + sums[a]) / area;
            let variance = (squares[d] - squares[b] - squares[c] + squares[a]) / area - mean * mean;
            if variance < 0.005_f64.powi(2) {
                values[y * width + x] = -2.0;
            }
        }
    }
    Ok(())
}

fn representation(image: &Mat, mode: &str) -> VisionResult<Mat> {
    let mode = match mode {
        "red-trio" => "red-green",
        "hybrid-trio" => "color-red",
        other => other,
    };
    if mode == "red-green" {
        // Suppress green/neutral background before smoothing. This is a color
        // feature experiment, not a general water or bobber segmentation.
        let mut feature = Mat::new_rows_cols_with_default(
            image.rows(),
            image.cols(),
            core::CV_32FC1,
            core::Scalar::all(0.0),
        )?;
        for y in 0..image.rows() {
            for x in 0..image.cols() {
                let pixel = image.at_2d::<core::Vec3b>(y, x)?;
                let red = pixel[2] as f32;
                let green = pixel[1] as f32;
                *feature.at_2d_mut::<f32>(y, x)? = ((red - green) / (red + green + 1.0)).max(0.0);
            }
        }
        let mut smooth = Mat::default();
        imgproc::gaussian_blur_def(&feature, &mut smooth, Size::new(5, 5), 1.0)?;
        return Ok(smooth);
    }
    if mode == "color-smooth"
        || mode == "chroma-smooth"
        || mode == "center-weighted"
        || mode == "color-red"
        || mode == "color-trio"
    {
        let mut smooth = Mat::default();
        imgproc::gaussian_blur_def(image, &mut smooth, Size::new(5, 5), 1.0)?;
        return representation(
            &smooth,
            if mode != "chroma-smooth" {
                "color"
            } else {
                "chroma"
            },
        );
    }
    if mode == "contrast" {
        let mut float = Mat::default();
        image.convert_to(&mut float, core::CV_32F, 1.0, 0.0)?;
        let mut background = Mat::default();
        imgproc::gaussian_blur_def(&float, &mut background, Size::new(0, 0), 3.0)?;
        let mut result = Mat::default();
        core::subtract(&float, &background, &mut result, &Mat::default(), -1)?;
        return Ok(result);
    }
    if mode == "chroma" {
        let mut lab = Mat::default();
        imgproc::cvt_color_def(image, &mut lab, imgproc::COLOR_BGR2Lab)?;
        let mut channels = core::Vector::<Mat>::new();
        core::split(&lab, &mut channels)?;
        let mut chroma = core::Vector::<Mat>::new();
        chroma.push(channels.get(1)?);
        chroma.push(channels.get(2)?);
        let mut result = Mat::default();
        core::merge(&chroma, &mut result)?;
        return Ok(result);
    }
    if mode == "color" {
        return Ok(image.try_clone()?);
    }
    let mut gray = Mat::default();
    imgproc::cvt_color_def(image, &mut gray, imgproc::COLOR_BGR2GRAY)?;
    if mode == "gray" {
        return Ok(gray);
    }
    let mut edges = Mat::default();
    let (low, high) = if mode == "edges-low" {
        (10.0, 25.0)
    } else {
        (50.0, 100.0)
    };
    imgproc::canny(&gray, &mut edges, low, high, 3, false)?;
    Ok(edges)
}

fn main() -> VisionResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(2..=3).contains(&args.len())
        || (args.len() == 3
            && !matches!(
                args[2].as_str(),
                "--diagnose"
                    | "--smooth"
                    | "--focus"
                    | "--foreground"
                    | "--hybrid"
                    | "--ensemble"
                    | "--ensemble-colors"
            ))
    {
        return Err(
            "Usage: vision-eval MANIFEST.csv NEW_OUTPUT.csv [--diagnose|--smooth|--focus|--foreground|--hybrid|--ensemble|--ensemble-colors]; fixed experiment for 1920x1080 images"
                .into(),
        );
    }
    let diagnose = args.len() == 3;
    let tri_color = args.get(2).is_some_and(|a| a == "--ensemble-colors");
    let ensemble = tri_color || args.get(2).is_some_and(|a| a == "--ensemble");
    let hybrid = args.get(2).is_some_and(|a| a == "--hybrid");
    let foreground = args.get(2).is_some_and(|a| a == "--foreground");
    let focus = ensemble || hybrid || foreground || args.get(2).is_some_and(|a| a == "--focus");
    let modes: &[&str] = if tri_color {
        &[
            "color-smooth",
            "red-green",
            "color-red",
            "color-trio",
            "red-trio",
            "hybrid-trio",
        ]
    } else if ensemble {
        &["color-smooth", "red-green", "color-red"]
    } else if hybrid {
        &["color-red"]
    } else if foreground {
        &["red-green"]
    } else if focus {
        &["color-smooth", "center-weighted"]
    } else if args.get(2).is_some_and(|a| a == "--smooth") {
        &["color-smooth", "chroma-smooth"]
    } else if diagnose {
        &["color", "contrast", "chroma", "edges-low"]
    } else {
        &["edges", "gray", "color"]
    };
    let regions = if diagnose {
        vec![("central", Rect::new(350, 150, 1250, 400))]
    } else {
        vec![
            ("full", Rect::new(0, 0, 1920, 1080)),
            ("central", Rect::new(350, 150, 1250, 400)),
        ]
    };
    let manifest = Path::new(&args[0]);
    let root = manifest.parent().ok_or("Missing parent directory")?;
    let text = fs::read_to_string(manifest)?;
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        let fields: Vec<_> = line.split(',').collect();
        if fields.len() != 7 {
            return Err("Expected seven CSV columns (no quoted commas)".into());
        }
        let rect = Rect::new(
            fields[3].parse()?,
            fields[4].parse()?,
            fields[5].parse()?,
            fields[6].parse()?,
        );
        let image = imgcodecs::imread(
            root.join(fields[0]).to_str().ok_or("Invalid path")?,
            imgcodecs::IMREAD_COLOR,
        )?;
        if image.cols() != 1920 || image.rows() != 1080 {
            return Err(format!("Expected 1920x1080: {}", fields[0]).into());
        }
        rows.push((
            fields[0].to_owned(),
            fields[1].to_owned(),
            fields[2].to_owned(),
            rect,
            image,
        ));
    }
    let mut templates = Vec::new();
    for (name, split, _, rect, image) in &rows {
        if split != "template" && !(focus && split == "evaluation-template") {
            continue;
        }
        let crop = Mat::roi(image, *rect)?.try_clone()?;
        for scale in [0.8, 1.0, 1.2] {
            let mut resized = Mat::default();
            imgproc::resize(
                &crop,
                &mut resized,
                Size::default(),
                scale,
                scale,
                imgproc::INTER_LINEAR,
            )?;
            for &mode in modes {
                let template = representation(&resized, mode)?;
                let mut mean = core::Scalar::default();
                let mut stddev = core::Scalar::default();
                core::mean_std_dev(&template, &mut mean, &mut stddev, &Mat::default())?;
                if (0..template.channels() as usize).all(|i| stddev[i] < 1e-6) {
                    continue;
                }
                let mask = if mode == "center-weighted" {
                    center_mask(template.size()?)?
                } else {
                    Mat::default()
                };
                let red_template = if matches!(mode, "color-red" | "hybrid-trio") {
                    let red = representation(&resized, "red-green")?;
                    core::mean_std_dev(&red, &mut mean, &mut stddev, &Mat::default())?;
                    if stddev[0] < 0.005 {
                        continue;
                    }
                    Some(red)
                } else {
                    None
                };
                templates.push((name.clone(), mode, scale, template, mask, red_template));
            }
        }
    }
    if templates.is_empty() {
        return Err("No nonconstant templates found".into());
    }
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?;
    writeln!(
        output,
        "file,split,present,mode,region,score,x,y,width,height,template,scale,center_in_target,iou,target_score"
    )?;
    for (name, split, present, target, image) in &rows {
        let colors = if tri_color {
            Some(ColorMap::from_bgr(image, TriColorConfig::default())?)
        } else {
            None
        };
        for &mode in modes {
            let frame = representation(image, mode)?;
            let red_frame = if matches!(mode, "color-red" | "hybrid-trio") {
                Some(representation(image, "red-green")?)
            } else {
                None
            };
            for &(region_name, region) in &regions {
                let search = Mat::roi(&frame, region)?;
                let mut best = (-2.0, Rect::default(), String::new(), 0.0);
                let mut target_score = -2.0_f64;
                for (template_name, template_mode, scale, template, mask, red_template) in
                    &templates
                {
                    if *template_mode != mode {
                        continue;
                    }
                    // Never count a source screenshot matching its own crop.
                    if focus && template_name == name {
                        continue;
                    }
                    let mut scores = Mat::default();
                    imgproc::match_template(
                        &search,
                        template,
                        &mut scores,
                        imgproc::TM_CCOEFF_NORMED,
                        mask,
                    )?;
                    sanitize_scores(&mut scores)?;
                    if let (Some(red_image), Some(red_template)) = (&red_frame, red_template) {
                        let red_search = Mat::roi(red_image, region)?;
                        let mut red_scores = Mat::default();
                        imgproc::match_template(
                            &red_search,
                            red_template,
                            &mut red_scores,
                            imgproc::TM_CCOEFF_NORMED,
                            &Mat::default(),
                        )?;
                        sanitize_scores(&mut red_scores)?;
                        reject_flat_patches(&mut red_scores, &red_search, red_template.size()?)?;
                        let mut combined = Mat::default();
                        core::add_weighted(&scores, 0.5, &red_scores, 0.5, 0.0, &mut combined, -1)?;
                        scores = combined;
                    }
                    if matches!(mode, "red-green" | "red-trio") {
                        reject_flat_patches(&mut scores, &search, template.size()?)?;
                    }
                    if mode.ends_with("-trio") {
                        let colors = colors.as_ref().ok_or("Missing color evidence")?;
                        let width = scores.cols() as usize;
                        for (index, score) in scores.data_typed_mut::<f32>()?.iter_mut().enumerate()
                        {
                            let window = Rect::new(
                                region.x + (index % width) as i32,
                                region.y + (index / width) as i32,
                                template.cols(),
                                template.rows(),
                            );
                            if !colors.evidence(window)?.passed {
                                *score = -2.0;
                            }
                        }
                    }
                    // Diagnostic only: labels never influence the predicted best location.
                    if present == "1" {
                        let x0 = (target.x - region.x - template.cols() / 2).max(0);
                        let y0 = (target.y - region.y - template.rows() / 2).max(0);
                        let x1 = (target.x + target.width - region.x - template.cols() / 2)
                            .min(scores.cols());
                        let y1 = (target.y + target.height - region.y - template.rows() / 2)
                            .min(scores.rows());
                        if x1 > x0 && y1 > y0 {
                            let local = Mat::roi(&scores, Rect::new(x0, y0, x1 - x0, y1 - y0))?;
                            let mut value = 0.0;
                            core::min_max_loc(
                                &local,
                                None,
                                Some(&mut value),
                                None,
                                None,
                                &Mat::default(),
                            )?;
                            if value.is_finite() {
                                target_score = target_score.max(value);
                            }
                        }
                    }
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
                            Rect::new(
                                point.x + region.x,
                                point.y + region.y,
                                template.cols(),
                                template.rows(),
                            ),
                            template_name.clone(),
                            *scale,
                        );
                    }
                }
                let r = best.1;
                let hit = present == "1"
                    && target.contains(Point::new(r.x + r.width / 2, r.y + r.height / 2));
                let iw = (i32::min(r.x + r.width, target.x + target.width)
                    - i32::max(r.x, target.x))
                .max(0);
                let ih = (i32::min(r.y + r.height, target.y + target.height)
                    - i32::max(r.y, target.y))
                .max(0);
                let intersection = iw * ih;
                let union = r.area() + target.area() - intersection;
                let iou = if union > 0 {
                    intersection as f64 / union as f64
                } else {
                    0.0
                };
                writeln!(
                    output,
                    "{name},{split},{present},{mode},{region_name},{:.6},{},{},{},{},{},{},{hit},{iou:.4},{target_score:.6}",
                    best.0, r.x, r.y, r.width, r.height, best.2, best.3
                )?;
            }
        }
        println!("Evaluated {name} ({split})");
    }
    Ok(())
}
