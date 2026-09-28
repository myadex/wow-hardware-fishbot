//! Shared motion measurement for the bot and the HID-free live bite test.
use crate::VisionResult;
use opencv::{
    core::{self, Mat, Rect},
    imgproc,
    prelude::*,
};

pub fn monitor_rect(rect: Rect) -> Rect {
    // Keep the established inner window for the water-bordered video crops.
    if (30..=48).contains(&rect.width) && (30..=48).contains(&rect.height) {
        Rect::new(rect.x + 3, rect.y + 3, rect.width - 6, rect.height - 6)
    } else {
        rect
    }
}

pub struct SplashMeasurement {
    pub changed_pixels: i32,
    pub total_pixels: i32,
}

impl SplashMeasurement {
    pub fn required_pixels(&self) -> i32 {
        (self.total_pixels as f64 * 0.12).ceil() as i32
    }

    pub fn is_candidate(&self) -> bool {
        self.changed_pixels >= self.required_pixels()
    }
}

/// Count pixels changing by more than 20 gray levels in the fixed bobber ROI.
/// Motion is a candidate, not confirmation that a fish bit.
pub fn measure_splash(
    previous: &Mat,
    current: &Mat,
    rect: Rect,
) -> VisionResult<SplashMeasurement> {
    if previous.empty()
        || current.empty()
        || previous.typ() != core::CV_8UC1
        || current.typ() != core::CV_8UC1
        || previous.size()? != current.size()?
        || rect.width <= 0
        || rect.height <= 0
    {
        return Err(
            "Bite measurement requires matching grayscale frames and a positive ROI".into(),
        );
    }
    // Difference only the monitored pixels; results equal the old full-frame
    // difference followed by cropping, without processing the rest of HDMI.
    let mut diff = Mat::default();
    core::absdiff(
        &Mat::roi(previous, rect)?,
        &Mat::roi(current, rect)?,
        &mut diff,
    )?;
    let mut mask = Mat::default();
    imgproc::threshold(&diff, &mut mask, 20.0, 255.0, imgproc::THRESH_BINARY)?;
    Ok(SplashMeasurement {
        changed_pixels: core::count_non_zero(&mask)?,
        total_pixels: rect.area(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_gray_threshold_and_offset_roi_ignore_outside_motion() -> VisionResult<()> {
        let previous =
            Mat::new_rows_cols_with_default(20, 20, core::CV_8UC1, core::Scalar::all(0.0))?;
        let mut current = previous.try_clone()?;
        imgproc::rectangle(
            &mut current,
            Rect::new(0, 0, 10, 10),
            core::Scalar::all(255.0),
            -1,
            imgproc::LINE_8,
            0,
        )?;
        imgproc::rectangle(
            &mut current,
            Rect::new(10, 10, 10, 10),
            core::Scalar::all(20.0),
            -1,
            imgproc::LINE_8,
            0,
        )?;
        let rect = Rect::new(10, 10, 10, 10);
        assert_eq!(measure_splash(&previous, &current, rect)?.changed_pixels, 0);
        for x in 10..20 {
            *current.at_2d_mut::<u8>(10, x)? = 21;
        }
        *current.at_2d_mut::<u8>(11, 10)? = 21;
        assert!(!measure_splash(&previous, &current, rect)?.is_candidate());
        *current.at_2d_mut::<u8>(11, 11)? = 21;
        let measure = measure_splash(&previous, &current, rect)?;
        assert_eq!(measure.changed_pixels, 12);
        assert!(measure.is_candidate());
        assert!(measure_splash(&previous, &current, Rect::new(19, 19, 5, 5)).is_err());
        assert!(measure_splash(&Mat::default(), &current, rect).is_err());
        Ok(())
    }
}
