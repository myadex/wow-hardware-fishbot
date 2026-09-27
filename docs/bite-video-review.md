# Local bite-video review

Two user recordings under ignored `samples/` show a visible bobber and fishing
line. The user confirmed a bite at about 8 seconds in `Aufzeichnung 2026-09-25
165123.mp4` and at 17.6–18.0 seconds in `Aufzeichnung 2026-09-25 165221.mp4`.
The clips are cropped to 1012×744 and 948×618 at 30 fps, respectively; they
are not full-resolution HDMI capture from the Pi.

At both confirmed bites, the bobber changes and a short circular ripple
expands around it. The first event is clearest around 8.0–8.4 seconds; the
second around 17.57–18.0 seconds. One-second sampling hides most of this motion,
so `video-sample` can extract frames every 0.1 seconds for visual review.

`video-motion` measures consecutive grayscale frames in a manually placed
27×27 bobber rectangle. Its CSV includes both the former >50 gray-level pixel
count and the new >20 gray-level count. The old trigger required more than 250
changed pixels; the largest measured counts at >50 were only 7 and 59. The new
trigger requires >=12% of the bobber rectangle to change by >20. With the
reviewed rectangles this means at least 88 of 729 pixels. The first event peaks
at 137, the second at 288. Before the respective bites, the maxima are 40 and
77. The first trigger frames are 8.000 and 17.567 seconds, with no trigger
before either confirmed event. On the second recording, brief ordinary motion
at 10.833 seconds reaches 77/729, close to the new limit; different capture
quality may need retuning.

The bot now reads one frame per monitoring step instead of discarding five.
The 50 ms pause remains. A 30-fps replay sampled every third frame still has a
candidate at every possible sampling phase in both clips; sampling every fifth
frame can miss the shorter first event. This is a timing argument based on these
recordings, not a hardware timing measurement.

For reproducible local diagnostics (the recordings and CSV results stay ignored):

```powershell
cargo run --locked --release --bin video-sample -- "samples/Aufzeichnung 2026-09-25 165221.mp4" output/video-2-event 0.1 17 19.5
cargo run --locked --release --bin video-motion -- "samples/Aufzeichnung 2026-09-25 165123.mp4" output/video-1-motion-new.csv 290 241 27 27
cargo run --locked --release --bin video-motion -- "samples/Aufzeichnung 2026-09-25 165221.mp4" output/video-2-motion-new.csv 281 323 27 27
```

The manually placed rectangles are for replay only. At runtime the detector's
bobber rectangle is used. **Bobber localization remains a separate limitation:**
the current edge matcher at threshold 0.80 did not accept still frames from
either cropped recording. A separate central-water smoothed-color probe with
video-derived crops selected the bobber correctly in four reviewed frames with
scores 0.796–0.887; the six labeled no-bobber screenshots scored below 0.721.
That probe is offline and has not replaced the bot's locator. Its thresholds
and templates need validation on full-resolution Pi capture before rollout.

These two confirmed bites support the new motion thresholds, but do not measure
false-positive rate across other water scenes or actual keyboard latency on the
Pi. The bot still sends F8 for a bite; that HID behavior needs the hardware run.
