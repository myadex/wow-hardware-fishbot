//! Offline diagnostic for a dark fishing line extending below a bobber candidate.
//! No reference labels are used to score predicted windows.
use opencv::{
    core::{Mat, Rect},
    imgcodecs,
    prelude::*,
};
use std::{collections::HashMap, fs, io::Write, path::Path};
use wow_hardware_fishbot::VisionResult;

fn line_score(gray: &Mat, rect: Rect) -> VisionResult<f64> {
    let width = gray.cols();
    let height = gray.rows();
    if rect.width <= 0
        || rect.height <= 0
        || rect.x < 0
        || rect.y < 0
        || rect.x + rect.width >= width
        || rect.y + rect.height + 55 >= height
    {
        return Ok(0.0);
    }
    let pixels = gray.data_typed::<u8>()?;
    let mut best: f64 = 0.0;
    let center = rect.x + rect.width / 2;
    for offset in -5..=5 {
        for slope_step in -16..=16 {
            let slope = slope_step as f64 * 0.05;
            let mut hits = 0;
            for dy in 5..=55 {
                let y = rect.y + rect.height + dy;
                let x = center + offset + (slope * dy as f64).round() as i32;
                if x < 9 || x + 9 >= width {
                    continue;
                }
                let index = y as usize * width as usize + x as usize;
                let center_value =
                    pixels[index - 1..=index + 1].iter().copied().min().unwrap() as i32;
                let side_value = (pixels[index - 8] as i32 + pixels[index + 8] as i32) / 2;
                if center_value + 8 < side_value && center_value < 90 {
                    hits += 1;
                }
            }
            best = best.max(hits as f64 / 51.0);
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencv::{
        core::{self, Point, Scalar},
        imgproc,
    };

    #[test]
    fn follows_slanted_dark_line_below_bobber_but_not_horizontal_wave() -> VisionResult<()> {
        let mut frame =
            Mat::new_rows_cols_with_default(120, 120, core::CV_8UC1, Scalar::all(100.0))?;
        let bobber = Rect::new(50, 20, 20, 15);
        imgproc::line(
            &mut frame,
            Point::new(60, 40),
            Point::new(82, 90),
            Scalar::all(10.0),
            1,
            imgproc::LINE_8,
            0,
        )?;
        assert!(line_score(&frame, bobber)? > 0.7);
        let mut wave =
            Mat::new_rows_cols_with_default(120, 120, core::CV_8UC1, Scalar::all(100.0))?;
        imgproc::line(
            &mut wave,
            Point::new(0, 60),
            Point::new(119, 60),
            Scalar::all(10.0),
            1,
            imgproc::LINE_8,
            0,
        )?;
        assert!(line_score(&wave, bobber)? < 0.2);
        Ok(())
    }
}

fn main() -> VisionResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: line-profile MANIFEST.csv PREDICTIONS.csv NEW_OUTPUT.csv".into());
    }
    let manifest = Path::new(&args[0]);
    let root = manifest.parent().ok_or("Missing manifest directory")?;
    let predictions = fs::read_to_string(&args[1])?;
    let mut grouped: HashMap<&str, Vec<Vec<&str>>> = HashMap::new();
    for line in predictions.lines().skip(1) {
        let fields: Vec<_> = line.split(',').collect();
        if fields.len() < 10 {
            return Err("Invalid prediction row".into());
        }
        grouped.entry(fields[0]).or_default().push(fields);
    }
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])?;
    writeln!(output, "file,kind,score,x,y,width,height")?;
    for line in fs::read_to_string(manifest)?.lines().skip(1) {
        let fields: Vec<_> = line.split(',').collect();
        if fields.len() != 7 {
            return Err("Invalid manifest row".into());
        }
        let frame = imgcodecs::imread(
            root.join(fields[0]).to_str().ok_or("Invalid image path")?,
            imgcodecs::IMREAD_GRAYSCALE,
        )?;
        if frame.empty() {
            return Err(format!("Unreadable frame: {}", fields[0]).into());
        }
        let mut windows = Vec::new();
        if fields[2] == "1" {
            windows.push((
                "reference",
                Rect::new(
                    fields[3].parse()?,
                    fields[4].parse()?,
                    fields[5].parse()?,
                    fields[6].parse()?,
                ),
            ));
        }
        if let Some(rows) = grouped.get(fields[0]) {
            for row in rows {
                windows.push((
                    row[3],
                    Rect::new(
                        row[6].parse()?,
                        row[7].parse()?,
                        row[8].parse()?,
                        row[9].parse()?,
                    ),
                ));
            }
        }
        for (kind, rect) in windows {
            let score = line_score(&frame, rect)?;
            writeln!(
                output,
                "{},{kind},{score:.6},{},{},{},{}",
                fields[0], rect.x, rect.y, rect.width, rect.height
            )?;
        }
    }
    Ok(())
}
