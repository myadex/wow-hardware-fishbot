//! Co-occurrence of three color bands in a candidate window. No target labels.
use crate::VisionResult;
use opencv::{
    core::{CV_8UC3, Mat, Rect, Vec3b},
    imgproc,
    prelude::*,
};

#[derive(Clone, Copy, Debug)]
pub struct ColorBand {
    /// OpenCV hue: 0..179. A reversed interval wraps through red at zero.
    pub hue_start: u8,
    pub hue_end: u8,
    pub min_saturation: u8,
    pub min_value: u8,
    pub max_value: u8,
}

impl ColorBand {
    fn contains(self, hsv: Vec3b) -> bool {
        let hue = if self.hue_start <= self.hue_end {
            hsv[0] >= self.hue_start && hsv[0] <= self.hue_end
        } else {
            hsv[0] >= self.hue_start || hsv[0] <= self.hue_end
        };
        hue && hsv[1] >= self.min_saturation && hsv[2] >= self.min_value && hsv[2] <= self.max_value
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TriColorConfig {
    pub brown: ColorBand,
    pub red: ColorBand,
    pub blue: ColorBand,
    pub min_pixels_per_color: u32,
}

impl Default for TriColorConfig {
    fn default() -> Self {
        Self {
            brown: ColorBand {
                hue_start: 8,
                hue_end: 30,
                min_saturation: 40,
                min_value: 20,
                max_value: 220,
            },
            red: ColorBand {
                hue_start: 170,
                hue_end: 7,
                min_saturation: 70,
                min_value: 20,
                max_value: 255,
            },
            blue: ColorBand {
                hue_start: 90,
                hue_end: 135,
                min_saturation: 35,
                min_value: 15,
                max_value: 255,
            },
            min_pixels_per_color: 2,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ColorEvidence {
    pub brown: u32,
    pub red: u32,
    pub blue: u32,
    pub passed: bool,
}

/// Prefix sums make each candidate-window check constant time.
pub struct ColorMap {
    width: i32,
    height: i32,
    sums: Vec<[u32; 3]>,
    minimum: u32,
}

impl ColorMap {
    pub fn from_bgr(image: &Mat, config: TriColorConfig) -> VisionResult<Self> {
        if image.empty() || image.typ() != CV_8UC3 {
            return Err("Color evidence needs a nonempty 8-bit BGR image".into());
        }
        if config.min_pixels_per_color == 0 {
            return Err("Color evidence needs at least one pixel per color".into());
        }
        let bands = [config.brown, config.red, config.blue];
        if bands
            .iter()
            .any(|b| b.hue_start > 179 || b.hue_end > 179 || b.min_value > b.max_value)
        {
            return Err("Invalid HSV color band".into());
        }
        let mut hsv = Mat::default();
        imgproc::cvt_color_def(image, &mut hsv, imgproc::COLOR_BGR2HSV)?;
        let width = image.cols();
        let height = image.rows();
        let stride = width as usize + 1;
        let mut sums = vec![[0; 3]; stride * (height as usize + 1)];
        let pixels = hsv.data_typed::<Vec3b>()?;
        for y in 0..height as usize {
            let mut row = [0; 3];
            for x in 0..width as usize {
                let pixel = pixels[y * width as usize + x];
                for color in 0..3 {
                    row[color] += u32::from(bands[color].contains(pixel));
                    sums[(y + 1) * stride + x + 1][color] =
                        sums[y * stride + x + 1][color] + row[color];
                }
            }
        }
        Ok(Self {
            width,
            height,
            sums,
            minimum: config.min_pixels_per_color,
        })
    }

    pub fn evidence(&self, rect: Rect) -> VisionResult<ColorEvidence> {
        if rect.x < 0
            || rect.y < 0
            || rect.width <= 0
            || rect.height <= 0
            || rect.x as i64 + rect.width as i64 > self.width as i64
            || rect.y as i64 + rect.height as i64 > self.height as i64
        {
            return Err("Color window is outside the image or empty".into());
        }
        let stride = self.width as usize + 1;
        let a = rect.y as usize * stride + rect.x as usize;
        let b = a + rect.width as usize;
        let c = a + rect.height as usize * stride;
        let d = c + rect.width as usize;
        let counts: [u32; 3] = std::array::from_fn(|i| {
            (self.sums[d][i] as i64 + self.sums[a][i] as i64
                - self.sums[b][i] as i64
                - self.sums[c][i] as i64) as u32
        });
        Ok(ColorEvidence {
            brown: counts[0],
            red: counts[1],
            blue: counts[2],
            passed: counts.iter().all(|n| *n >= self.minimum),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencv::core::Scalar;

    #[test]
    fn three_colors_must_cooccur_in_the_same_window() -> VisionResult<()> {
        let mut image = Mat::new_rows_cols_with_default(4, 12, CV_8UC3, Scalar::all(0.0))?;
        for y in 1..3 {
            *image.at_2d_mut::<Vec3b>(y, 1)? = Vec3b::from([20, 60, 100]);
            *image.at_2d_mut::<Vec3b>(y, 4)? = Vec3b::from([20, 20, 130]);
            *image.at_2d_mut::<Vec3b>(y, 8)? = Vec3b::from([120, 40, 20]);
        }
        let map = ColorMap::from_bgr(&image, TriColorConfig::default())?;
        let all = map.evidence(Rect::new(0, 0, 12, 4))?;
        assert_eq!((all.brown, all.red, all.blue), (2, 2, 2));
        assert!(all.passed);
        assert!(!map.evidence(Rect::new(0, 0, 7, 4))?.passed);
        assert!(map.evidence(Rect::new(-1, 0, 2, 2)).is_err());
        Ok(())
    }

    #[test]
    fn dark_noise_is_not_blue_evidence_and_red_hue_wraps() {
        let cfg = TriColorConfig::default();
        assert!(!cfg.blue.contains(Vec3b::from([110, 255, 4])));
        assert!(cfg.red.contains(Vec3b::from([179, 180, 100])));
        assert!(cfg.red.contains(Vec3b::from([0, 180, 100])));
        assert!(!cfg.red.contains(Vec3b::from([90, 180, 100])));
    }
}
