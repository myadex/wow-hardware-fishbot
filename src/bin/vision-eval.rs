//! Offline experiment. Does not change production detection or use HID devices.
use opencv::{
    core::{self, Mat, Point, Rect, Size},
    imgcodecs, imgproc,
    prelude::*,
};
use std::{fs, io::Write, path::Path};
use wow_hardware_fishbot::VisionResult;

fn representation(image: &Mat, mode: &str) -> VisionResult<Mat> {
    if mode == "color-smooth" || mode == "chroma-smooth" {
        let mut smooth = Mat::default();
        imgproc::gaussian_blur_def(image, &mut smooth, Size::new(5, 5), 1.0)?;
        return representation(
            &smooth,
            if mode == "color-smooth" {
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
        || (args.len() == 3 && !matches!(args[2].as_str(), "--diagnose" | "--smooth"))
    {
        return Err(
            "Usage: vision-eval MANIFEST.csv NEW_OUTPUT.csv [--diagnose|--smooth]; fixed experiment for 1920x1080 images"
                .into(),
        );
    }
    let diagnose = args.len() == 3;
    let modes: &[&str] = if args.get(2).is_some_and(|a| a == "--smooth") {
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
        if split != "template" {
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
                templates.push((name.clone(), mode, scale, template));
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
        for &mode in modes {
            let frame = representation(image, mode)?;
            for &(region_name, region) in &regions {
                let search = Mat::roi(&frame, region)?;
                let mut best = (-2.0, Rect::default(), String::new(), 0.0);
                let mut target_score = -2.0_f64;
                for (template_name, template_mode, scale, template) in &templates {
                    if *template_mode != mode {
                        continue;
                    }
                    let mut scores = Mat::default();
                    imgproc::match_template(
                        &search,
                        template,
                        &mut scores,
                        imgproc::TM_CCOEFF_NORMED,
                        &Mat::default(),
                    )?;
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
