use opencv::{
    core::{Scalar, Vector},
    imgcodecs, imgproc,
    prelude::*,
};
use std::path::Path;
use wow_hardware_fishbot::{VisionResult, find_bobber, load_templates};

fn main() -> VisionResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h") {
        println!(
            "image-test INPUT OUTPUT [TEMPLATE_DIRECTORY] [THRESHOLD]\nDefault templates: templates; threshold: 0.80 (uncalibrated).\nExit codes: 0 = match, 2 = no match, 1 = error. No capture or HID devices are used."
        );
        return Ok(());
    }
    if !(2..=4).contains(&args.len()) {
        return Err("Usage: image-test INPUT OUTPUT [TEMPLATE_DIRECTORY] [THRESHOLD]".into());
    }
    let input = Path::new(&args[0]);
    let output = Path::new(&args[1]);
    // Create-new prevents accidentally overwriting source images or prior results.
    if output.exists() {
        return Err("Output already exists; choose a new filename".into());
    }
    let templates = load_templates(Path::new(
        args.get(2).map(String::as_str).unwrap_or("templates"),
    ))?;
    let threshold = args
        .get(3)
        .map(|s| s.parse::<f64>())
        .transpose()?
        .unwrap_or(0.80);
    let mut image = imgcodecs::imread(
        input.to_str().ok_or("Invalid input path")?,
        imgcodecs::IMREAD_COLOR,
    )?;
    if image.empty() {
        return Err("Input image could not be read".into());
    }
    let mut gray = opencv::core::Mat::default();
    imgproc::cvt_color_def(&image, &mut gray, imgproc::COLOR_BGR2GRAY)?;
    let found = find_bobber(&gray, &templates, threshold)?;
    if let Some(ref detection) = found {
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
            &mut image,
            detection.rect,
            Scalar::new(0.0, 255.0, 0.0, 0.0),
            2,
            imgproc::LINE_8,
            0,
        )?;
    } else {
        println!("NO MATCH at threshold {threshold:.4}");
    }
    let extension = output
        .extension()
        .and_then(|e| e.to_str())
        .ok_or("Output needs an image extension, e.g. .png")?;
    let mut encoded = Vector::<u8>::new();
    if !imgcodecs::imencode(
        &format!(".{extension}"),
        &image,
        &mut encoded,
        &Vector::new(),
    )? {
        return Err("Image encoding failed".into());
    }
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(encoded.as_slice())?;
    println!("Output: {}", output.display());
    if found.is_none() {
        std::process::exit(2);
    }
    Ok(())
}
