# Color-first bobber localization

The Pi bot and Windows `image-test` now use the same two-stage locator. It first
searches the central area (15–87% of width, 12–64% of height) with nine cropped
color templates in `color-templates/`, at 0.8, 1.0 and 1.2 scale. Both frame and
templates receive a 5×5 Gaussian blur before normalized correlation. A score of
at least 0.78 accepts the best color match. If none qualifies, the previous
Canny edge matcher searches with `templates/` at 0.80. Correlation scores from
these two methods are not probabilities or directly comparable.

Six crops come from user-confirmed 15xx/17xx screenshots. Three come from the
two user recordings, including the later camera view in `165123.mp4`. Only the
small PNG crops are committed; the original screenshots, recordings and
diagnostic output remain in ignored local directories. The three-color and
fishing-line channels in `vision-eval` remain experiments and are not part of
this real-time locator.

On the inspected screenshot development set there are 16 labeled positives and
9 negatives. Seven positive screenshots are sources of a shipped crop (six
color, one edge) and are excluded from the generalization count. Of the other
9 positives, the combined locator puts the match on the bobber in 8; it misses
171459. All 9 labeled negatives are rejected. This is a same-session
development check, not independent accuracy. Across 18 additional reviewed
video frames from the same two recordings, all matches are on the bobber; two
frames before a bobber appears are rejected. Some video frames are close in
time to the template source, so this is a playback smoke test, not a held-out
recording. The highest raw color score on those two no-bobber video frames is
0.772327, only 0.007673 below the 0.78 threshold. New scenes could therefore
produce false matches.

The color matches from the videos are 32×32. Their water border dilutes the
12%-changed-pixel bite rule, so the bot monitors the inner 26×26 rectangle for
these matches. Offline replay with these rectangles detects the three confirmed
bites at 8.000, 36.333 and 17.567 seconds. Camera movement between casts is
not modeled as a fixed bobber location. Actual Pi frame timing and the full
HDMI capture format are still untested.

For the bite action, the relative mouse is parked at the top-left before each
cast so it does not cover the bobber while motion is measured. On a detected
bite, the center of the matched rectangle is scaled to the configured game
desktop size, then the cursor moves there and sends a right-click. This physical
positioning is not validated by offline video playback.

On the configured Windows machine, the same production path can be checked
without capture hardware or HID input:

```powershell
. .\scripts\windows-env.ps1
cargo test --locked --all-targets
cargo run --locked --release --bin image-test -- samples\WoWScrnShot_092526_151716.jpg output\151716-color-review.jpg
cargo run --locked --release --bin image-test -- output\video-2-samples\frame-0017.jpg output\video2-color-review.jpg
```

The output files must not already exist. A green rectangle marks the accepted
location; exit code 2 means no match. `color-image-test` returns the best raw
color score without imposing the 0.78 threshold, which helps inspect misses.
