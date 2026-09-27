# Local bite-video review

Two user recordings under ignored `samples/` show a visible bobber and fishing
line. The user confirmed two bites in `Aufzeichnung 2026-09-25 165123.mp4`, at
about 8 seconds and 35–37 seconds, and one at 17.6–18.0 seconds in
`Aufzeichnung 2026-09-25 165221.mp4`.
The clips are cropped to 1012×744 and 948×618 at 30 fps, respectively; they
are not full-resolution HDMI capture from the Pi.

At all three confirmed bites, the bobber changes and a short circular ripple
expands around it. The events are clearest around 8.0–8.4, 36.33–36.8, and
17.57–18.0 seconds, respectively. One-second sampling hides most of this motion,
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

For the later bite in the first recording, the camera has moved and the bobber
is near (430, 251). A separate 27×27 rectangle at (418, 238) first triggers at
36.333 seconds and peaks at 155/729 pixels at 36.400 seconds. From 13.0 to
36.3 seconds, after the camera settles and before this bite, the maximum is
71/729, below the 88-pixel limit. The old >50 count peaks at only 10 during
this bite. Running that later rectangle over the *entire* clip also reports a
candidate at 11.433 seconds because the camera is moving through its future
position; it is not a valid fixed-bobber replay before the later cast.

The bot now reads one frame per monitoring step instead of discarding five.
The 50 ms pause remains. A 30-fps replay sampled every third frame still has a
candidate at every possible sampling phase for the first two reviewed events;
sampling every fifth frame can miss the shorter first event. The 36-second bite
has candidates on frames 1090, 1092, 1093, and 1096, so one of three possible
every-third-frame phases would miss it. This is a timing argument based on these
recordings, not a hardware timing measurement.

For reproducible local diagnostics (the recordings and CSV results stay ignored):

```powershell
cargo run --locked --release --bin video-sample -- "samples/Aufzeichnung 2026-09-25 165221.mp4" output/video-2-event 0.1 17 19.5
cargo run --locked --release --bin video-motion -- "samples/Aufzeichnung 2026-09-25 165123.mp4" output/video-1-motion-new.csv 290 241 27 27
cargo run --locked --release --bin video-motion -- "samples/Aufzeichnung 2026-09-25 165123.mp4" output/video-1-second-motion.csv 418 238 27 27
cargo run --locked --release --bin video-motion -- "samples/Aufzeichnung 2026-09-25 165221.mp4" output/video-2-motion-new.csv 281 323 27 27
```

The manually placed rectangles above are for diagnostic replay. The bot now
uses a smoothed-color locator with nine shipped crops, then the original edge
matcher as fallback. For 32×32 color matches it monitors the inner 26×26 region
to avoid diluting short motion with the water border. Replaying the three bites
with the actual inner rectangles first triggers at 8.000, 36.333 and 17.567
seconds. The new locator finds 18 reviewed bobber video frames, including both
camera positions in the first recording, and rejects two reviewed frames before
the bobber appears in the second. See [the locator review](color-locator.md).

These three confirmed bites support the motion thresholds, but do not measure
false-positive rate across other water scenes or actual mouse HID latency on the
Pi. The bot now moves to the bobber and right-clicks at a bite; that physical
pointer position and click need the hardware run.
