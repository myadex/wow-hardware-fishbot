//! Offline raw-score probe for the same color locator used by the bot.
//! No capture or HID devices.
use opencv::{imgcodecs, prelude::*};
use std::path::Path;
use wow_hardware_fishbot::{VisionResult, find_color_bobber, load_color_templates};

fn main() -> VisionResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: color-image-test INPUT_IMAGE TEMPLATE_DIRECTORY".into());
    }
    let image = imgcodecs::imread(&args[0], imgcodecs::IMREAD_COLOR)?;
    if image.empty() {
        return Err("Input image unreadable".into());
    }
    let templates = load_color_templates(Path::new(&args[1]))?;
    let best = find_color_bobber(&image, &templates, 0.0)?.ok_or("No fitting color template")?;
    println!(
        "score={:.6} template={} x={} y={} width={} height={}",
        best.score, best.template, best.rect.x, best.rect.y, best.rect.width, best.rect.height
    );
    Ok(())
}
