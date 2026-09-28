//! Live bite-motion check using production capture and detector, with no HID.
use opencv::{
    core::{Mat, Scalar, Vector},
    imgcodecs, imgproc,
    prelude::*,
};
use std::{
    fs,
    io::Write,
    path::Path,
    thread::sleep,
    time::{Duration, Instant},
};
use wow_hardware_fishbot::{
    VisionResult,
    bite::{measure_splash, monitor_rect},
    capture::LiveCapture,
    find_bobber_in_frame, load_color_templates, load_templates,
};

fn gray(frame: &Mat) -> VisionResult<Mat> {
    let mut result = Mat::default();
    imgproc::cvt_color_def(frame, &mut result, imgproc::COLOR_BGR2GRAY)?;
    Ok(result)
}

fn save_review(
    directory: &Path,
    name: &str,
    frame: &Mat,
    rect: opencv::core::Rect,
) -> VisionResult<()> {
    let mut review = frame.try_clone()?;
    imgproc::rectangle(
        &mut review,
        rect,
        Scalar::new(0.0, 255.0, 0.0, 0.0),
        2,
        imgproc::LINE_8,
        0,
    )?;
    let path = directory.join(name);
    if !imgcodecs::imwrite(
        path.to_str().ok_or("Output path is not UTF-8")?,
        &review,
        &Vector::new(),
    )? {
        return Err("Could not save bite review image".into());
    }
    Ok(())
}

fn main() -> VisionResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h") {
        println!(
            "bite-test NEW_OUTPUT_DIRECTORY [SECONDS]\nDefault: 25 seconds. Cast manually before starting. Keep the camera still and do not loot/recast during measurement. No HID devices are opened. Exit 0: candidates recorded, 2: no candidate/bobber, 1: error."
        );
        return Ok(());
    }
    if !(1..=2).contains(&args.len()) {
        return Err("Usage: bite-test NEW_OUTPUT_DIRECTORY [SECONDS]".into());
    }
    let seconds: u64 = args.get(1).map(|v| v.parse()).transpose()?.unwrap_or(25);
    if !(1..=120).contains(&seconds) {
        return Err("Duration must be 1..120 seconds".into());
    }
    let directory = Path::new(&args[0]);
    if directory.exists() {
        return Err("Choose a new output directory".into());
    }
    let edge_templates = load_templates(Path::new("templates"))?;
    let color_templates = load_color_templates(Path::new("color-templates"))?;
    let mut capture = LiveCapture::open()?;
    let initial = capture.fresh_frame()?;
    fs::create_dir(directory)?;
    let Some(detection) = find_bobber_in_frame(&initial, &edge_templates, &color_templates, 0.80)?
    else {
        let path = directory.join("no-bobber.png");
        if !imgcodecs::imwrite(
            path.to_str().ok_or("Invalid output path")?,
            &initial,
            &Vector::new(),
        )? {
            return Err("Could not save no-bobber frame".into());
        }
        println!(
            "NO MATCH; saved {}. Cast again and use a new output directory.",
            path.display()
        );
        std::process::exit(2);
    };
    let rect = monitor_rect(detection.rect);
    save_review(directory, "initial.png", &initial, rect)?;
    let mut csv = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("motion.csv"))?;
    writeln!(
        csv,
        "seconds,frame_gap_ms,changed_pixels,total_pixels,required_pixels,candidate"
    )?;
    let mut metadata = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("setup.txt"))?;
    writeln!(
        metadata,
        "template={} score={:.6}\nmonitor x={} y={} width={} height={}\nseconds={}\nFISHBOT_SWAP_RB={}",
        detection.template,
        detection.score,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        seconds,
        std::env::var("FISHBOT_SWAP_RB").unwrap_or_else(|_| "0".into())
    )?;
    let required = (rect.area() as f64 * 0.12).ceil() as i32;
    println!(
        "MATCH template={} score={:.4}; monitor x={} y={} {}x{}, threshold={required}/{} pixels",
        detection.template,
        detection.score,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        rect.area()
    );
    println!("Monitoring for {seconds}s. Motion candidates are recorded; no click will be sent.");
    // Template search and writing the initial review can take time. Start
    // from a fresh baseline so that delay does not count as bobber motion.
    let mut previous_bgr = capture.fresh_frame()?;
    let mut previous_gray = gray(&previous_bgr)?;
    let start = Instant::now();
    let mut previous_time = start;
    let mut last_report = start;
    let mut previous_candidate = false;
    let mut candidates = 0;
    let mut frames = 0;
    let mut peak = 0;
    let mut first_candidate = None;
    while start.elapsed() < Duration::from_secs(seconds) {
        let current_bgr = capture.next_frame()?;
        let now = Instant::now();
        let current_gray = gray(&current_bgr)?;
        let measurement = measure_splash(&previous_gray, &current_gray, rect)?;
        let candidate = measurement.is_candidate();
        writeln!(
            csv,
            "{:.3},{:.3},{},{},{},{}",
            now.duration_since(start).as_secs_f64(),
            now.duration_since(previous_time).as_secs_f64() * 1000.0,
            measurement.changed_pixels,
            measurement.total_pixels,
            measurement.required_pixels(),
            candidate
        )?;
        frames += 1;
        peak = peak.max(measurement.changed_pixels);
        if candidate {
            candidates += 1;
            if candidates == 1 {
                first_candidate = Some((previous_bgr.try_clone()?, current_bgr.try_clone()?));
            }
        }
        if candidate && !previous_candidate {
            println!(
                "BITE CANDIDATE at {:.3}s: {}/{} pixels changed",
                now.duration_since(start).as_secs_f64(),
                measurement.changed_pixels,
                measurement.total_pixels
            );
        } else if last_report.elapsed() >= Duration::from_secs(1) {
            println!(
                "{:.1}s: {}/{} pixels changed (peak {peak}, threshold {required})",
                start.elapsed().as_secs_f64(),
                measurement.changed_pixels,
                measurement.total_pixels
            );
            last_report = now;
        }
        previous_candidate = candidate;
        previous_bgr = current_bgr;
        previous_gray = current_gray;
        previous_time = now;
        sleep(Duration::from_millis(50));
    }
    csv.flush()?;
    // Save full PNGs after monitoring, so encoding does not stall comparison.
    if let Some((before, after)) = first_candidate {
        save_review(directory, "first-candidate-before.png", &before, rect)?;
        save_review(directory, "first-candidate-after.png", &after, rect)?;
    }
    println!(
        "Finished: {frames} comparisons, {candidates} candidate frames, peak={peak}, threshold={required}. Results: {}",
        directory.display()
    );
    if candidates == 0 {
        std::process::exit(2);
    }
    Ok(())
}
