//! Shared V4L2 capture for the bot and the HID-free hardware check.
use crate::VisionResult;
use opencv::{
    core::{self, Mat, Size},
    imgproc,
    prelude::*,
    videoio::{self, VideoCapture},
};

fn parse_swap_rb(value: Option<&str>) -> VisionResult<bool> {
    match value {
        None | Some("0") => Ok(false),
        Some("1") => Ok(true),
        _ => Err("FISHBOT_SWAP_RB must be 0 or 1".into()),
    }
}

fn normalize_frame(frame: Mat, swap_rb: bool) -> VisionResult<Mat> {
    if frame.empty() || frame.typ() != core::CV_8UC3 {
        return Err("Capture must produce a nonempty 8-bit three-channel frame".into());
    }
    if !swap_rb {
        return Ok(frame);
    }
    let mut corrected = Mat::default();
    imgproc::cvt_color_def(&frame, &mut corrected, imgproc::COLOR_RGB2BGR)?;
    Ok(corrected)
}

pub struct LiveCapture {
    video: VideoCapture,
    swap_rb: bool,
}

impl LiveCapture {
    /// Open the configured CSI pipeline. EDID and media links must already be set.
    pub fn open() -> VisionResult<Self> {
        let swap_value = std::env::var("FISHBOT_SWAP_RB").ok();
        let swap_rb = parse_swap_rb(swap_value.as_deref())?;
        let mut video = VideoCapture::new(0, videoio::CAP_V4L2)?;
        if !video.is_opened()? {
            return Err("Could not open /dev/video0; configure the HDMI pipeline first".into());
        }
        let rgb3 = videoio::VideoWriter::fourcc('R', 'G', 'B', '3')?;
        for (property, value) in [
            (videoio::CAP_PROP_FRAME_WIDTH, 1920.0),
            (videoio::CAP_PROP_FRAME_HEIGHT, 1080.0),
            (videoio::CAP_PROP_FOURCC, rgb3 as f64),
            (videoio::CAP_PROP_BUFFERSIZE, 3.0),
            (videoio::CAP_PROP_CONVERT_RGB, 1.0),
        ] {
            if !video.set(property, value)? {
                return Err(format!("Capture rejected property {property}={value}").into());
            }
        }
        let size = Size::new(
            video.get(videoio::CAP_PROP_FRAME_WIDTH)? as i32,
            video.get(videoio::CAP_PROP_FRAME_HEIGHT)? as i32,
        );
        if size != Size::new(1920, 1080) {
            return Err(format!(
                "Expected 1920x1080 capture, got {}x{}",
                size.width, size.height
            )
            .into());
        }
        if video.get(videoio::CAP_PROP_FOURCC)? as i32 != rgb3 {
            return Err("Capture did not accept RGB3".into());
        }
        println!(
            "Capture /dev/video0: 1920x1080 RGB3, FISHBOT_SWAP_RB={}",
            u8::from(swap_rb)
        );
        // The first single-frame hardware capture was partial. Discard the same
        // 30 startup frames that produced the user's complete HDMI test image.
        for _ in 0..30 {
            if !video.grab()? {
                return Err("Failed to grab an HDMI startup frame".into());
            }
        }
        Ok(Self { video, swap_rb })
    }

    pub fn dimensions(&self) -> VisionResult<Size> {
        Ok(Size::new(
            self.video.get(videoio::CAP_PROP_FRAME_WIDTH)? as i32,
            self.video.get(videoio::CAP_PROP_FRAME_HEIGHT)? as i32,
        ))
    }

    /// Read each next frame for bite monitoring, applying the same color setting.
    pub fn next_frame(&mut self) -> VisionResult<Mat> {
        let mut frame = Mat::default();
        if !self.video.read(&mut frame)? {
            return Err("Failed to read an HDMI frame".into());
        }
        normalize_frame(frame, self.swap_rb)
    }

    /// Drain buffered frames after a casting delay before locating the bobber.
    pub fn fresh_frame(&mut self) -> VisionResult<Mat> {
        for _ in 0..5 {
            if !self.video.grab()? {
                return Err("Failed to drain an HDMI frame".into());
            }
        }
        let mut frame = Mat::default();
        if !self.video.retrieve(&mut frame, 0)? {
            return Err("Failed to retrieve an HDMI frame".into());
        }
        normalize_frame(frame, self.swap_rb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_override_swaps_red_and_blue_without_changing_green() -> VisionResult<()> {
        let frame = Mat::new_rows_cols_with_default(
            1,
            1,
            core::CV_8UC3,
            core::Scalar::new(10.0, 20.0, 200.0, 0.0),
        )?;
        let unchanged = normalize_frame(frame.try_clone()?, false)?;
        assert_eq!(unchanged.at_2d::<core::Vec3b>(0, 0)?.0, [10, 20, 200]);
        let swapped = normalize_frame(frame, true)?;
        assert_eq!(swapped.at_2d::<core::Vec3b>(0, 0)?.0, [200, 20, 10]);
        Ok(())
    }

    #[test]
    fn rejects_invalid_capture_frames_and_settings() -> VisionResult<()> {
        assert!(normalize_frame(Mat::default(), true).is_err());
        let gray = Mat::new_rows_cols_with_default(1, 1, core::CV_8UC1, core::Scalar::all(0.0))?;
        assert!(normalize_frame(gray, false).is_err());
        assert!(!parse_swap_rb(None)?);
        assert!(!parse_swap_rb(Some("0"))?);
        assert!(parse_swap_rb(Some("1"))?);
        assert!(parse_swap_rb(Some("yes")).is_err());
        Ok(())
    }
}
