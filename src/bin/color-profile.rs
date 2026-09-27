use opencv::{core::Rect, imgcodecs};
use std::{fs, io::Write, path::Path};
use wow_hardware_fishbot::{
    VisionResult,
    colors::{ColorMap, TriColorConfig},
};

fn main() -> VisionResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: color-profile MANIFEST.csv PREDICTIONS.csv NEW_OUTPUT.csv".into());
    }
    let manifest = Path::new(&args[0]);
    let root = manifest.parent().ok_or("No manifest parent")?;
    let manifest_text = fs::read_to_string(manifest)?;
    let predictions = fs::read_to_string(&args[1])?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])?;
    writeln!(output, "file,kind,x,y,width,height,brown,red,blue,passed")?;
    for line in manifest_text.lines().skip(1) {
        let f: Vec<_> = line.split(',').collect();
        if f.len() != 7 {
            return Err("Invalid manifest row".into());
        }
        let image = imgcodecs::imread(
            root.join(f[0]).to_str().ok_or("Invalid image path")?,
            imgcodecs::IMREAD_COLOR,
        )?;
        let map = ColorMap::from_bgr(&image, TriColorConfig::default())?;
        let mut windows = Vec::new();
        if f[2] == "1" {
            windows.push((
                "reference".to_string(),
                Rect::new(f[3].parse()?, f[4].parse()?, f[5].parse()?, f[6].parse()?),
            ));
        }
        for row in predictions.lines().skip(1) {
            let p: Vec<_> = row.split(',').collect();
            if p.len() < 10 {
                return Err("Invalid predictions row".into());
            }
            if p[0] == f[0] {
                windows.push((
                    p[3].to_owned(),
                    Rect::new(p[6].parse()?, p[7].parse()?, p[8].parse()?, p[9].parse()?),
                ));
            }
        }
        for (kind, r) in windows {
            let e = map.evidence(r)?;
            writeln!(
                output,
                "{},{kind},{},{},{},{},{},{},{},{}",
                f[0], r.x, r.y, r.width, r.height, e.brown, e.red, e.blue, e.passed
            )?;
        }
    }
    Ok(())
}
