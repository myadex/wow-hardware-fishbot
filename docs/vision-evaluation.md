# Offline comparison experiment

`vision-eval` compares edge, grayscale, and BGR color normalized correlation.
It does not change the production detector. All methods use the same confirmed
template crops, resized by 0.8, 1.0, and 1.2. Each searches both the full frame
and a central rectangle `(350,150,1250,400)`. This rectangle excludes most UI,
but is not water segmentation and still includes some shore.

From a configured Windows PowerShell session:

```powershell
cargo run --locked --release --bin vision-eval -- samples/evaluation.csv output/vision-comparison.csv
```

The output must not exist. Input CSV has the header
`file,split,present,x,y,width,height`. Paths are relative to the CSV directory;
quoted fields/commas in filenames are not supported. Images must be 1920x1080.
Positive rows contain a confirmed rectangle, negatives use four zeroes.
Rows with split `template` supply the template crops. Other split names are
passed through to the results. Images and labels remain local under `samples`.

For the September 25 sample set, the user confirmed all eight positive boxes
and six negative frames. Template images are 151704, 151815, 151820; calibration
negatives are 151750, 151800, 151803. Evaluation uses the other five positives
and three negatives. Proposed threshold per method/region is its highest
calibration-negative score plus 0.05. A threshold above 1.0 means that channel
cannot safely accept any normalized correlation. A correct localization
requires the predicted center inside the confirmed box and IoU at least 0.30.
Accepted detections outside a positive target count as wrong localizations,
not successful detections. CSV records raw scores, positions, centers, and IoU;
it does not apply a threshold itself.

This is exploratory evaluation: the scene and images were inspected before
the split, and adjacent frames are strongly correlated. It is not an independent
accuracy benchmark. Template-image self-matches must not be counted as evaluation
successes. A future test needs a separate recording/session, including negatives.

## Diagnosing the missed targets

Append `--diagnose` to compare the original color method with local contrast
removal, Lab chroma-only matching, and lower Canny thresholds in the central
region. Append `--smooth` to compare Gaussian-smoothed BGR and chroma matching
(5x5 kernel, sigma 1.0). Scale factors and template source images stay fixed.
The `target_score` column is the highest score with a predicted center inside
the confirmed box. It is diagnostic only; target labels do not affect search or
template selection. Negative rows use -2 as the unavailable target-score sentinel.

```powershell
cargo run --locked --release --bin vision-eval -- samples/evaluation.csv output/smooth-new.csv --smooth
.\scripts\summarize-vision.ps1 output/smooth-new.csv | Format-Table
```

On the current local sample set, the color baseline selects background instead
of the target in 151728 and 151824. It finds the target but rejects its score in
151827 and 151852. Smoothing BGR resolves all four cases at the unchanged
calibration rule: threshold 0.780338, 5/5 correct localizations, 0/3 false alarms
on the evaluation rows. Scores for those positive rows range from 0.822317 to
0.936607; negative evaluation rows peak at 0.759503. Baseline color gave 1/5
correct localizations and 0/3 false alarms. Chroma, contrast removal, and lower
edge thresholds did not produce an equally successful result.

The former evaluation rows are now development data: this result was used to
select smoothing. It must not be reported as independent test accuracy. The
remaining score margin is small and requires testing on new footage. No hardware
bot behavior or default detection threshold is changed by this experiment.

## Foreground and additional views (September 27)

The eleven 17xx screenshots contain eight positives and three confirmed negatives
(171355, 171359, 171403). The previous frozen color method found 3/8 positives
and rejected all three negatives. Three missed positives preferred background
locations; two were correctly localized but below threshold. Reference boxes are
annotations, not predictions. The user confirmed the reference positions for the
three background-confusion cases.

New experimental options:

- `--focus`: compare smoothed color with a Gaussian spatial mask emphasizing the
  template center. This is weighting, not semantic foreground segmentation.
- `--foreground`: use positive `(R-G)/(R+G+1)` followed by 5x5 Gaussian smoothing.
  Reject search patches with feature standard deviation below 0.005: almost-flat
  patches can otherwise produce numerically misleading perfect correlations.
- `--hybrid`: average smoothed color and foreground correlations at the **same
  position for the same template**. Invalid/flat foreground patches are rejected.
- `--ensemble`: emit color, foreground, and hybrid scores for the summary below.

These options accept `evaluation-template` rows as additional template sources,
but exclude **all templates from the current source file**, including resized
variants, when evaluating it. Thus a source frame cannot win by matching its own
crop. Ordinary `template` rows are still excluded from reported success counts.
For this experiment the additional views are 171341, 171345, 171419. The three
original template sources, scale factors, search rectangle and calibration-negative
images remain unchanged. The 16xx images lack confirmed labels and are not scored.

```powershell
cargo run --locked --release --bin vision-eval -- samples/focus-expanded.csv output/ensemble-new.csv --ensemble
.\scripts\summarize-ensemble.ps1 -CsvPath output/ensemble-new.csv -OutputPath output/ensemble-new-summary.json
```

The summary calibrates each channel from the same three old negative images
(max score + 0.05). Any channel above its threshold can propose a detection;
the accepted candidate with the largest score-minus-threshold margin wins,
with ties ordered by mode name. Selection never uses evaluation presence labels,
reference boxes, or the diagnostic score at the reference box.

Development-set results, excluding the original template images and three
calibration negatives:

| Method | Correct / 13 positives | False alarms / 6 negatives |
|---|---:|---:|
| Original color, original views | 8 | 0 |
| Color, additional views with own-source exclusion | 8 | 0 |
| Center weighting | 2 | 0 |
| Foreground alone | 2 | 0 |
| Hybrid alone | 7 | 0 |
| Three-channel ensemble | 10 | 0 |

The ensemble recovers 171345 and 171459, raising the 17xx result from 3/8 to 5/8.
171341, 171419 and 171431 are still missed. All five evaluated 15xx positives
remain detected. Calibrated thresholds are 0.780338 (color), 0.926487 (foreground),
and 0.827874 (hybrid). Center weighting was rejected as an improvement.

This is development evidence only: the added views and ensemble were selected
after inspecting these frames. Own-source exclusion prevents trivial self-matches,
but does not create an independent test or remove correlation between adjacent
frames. The hardware bot and `image-test` still use the earlier detector.

Validation includes the two numeric regression tests in `vision-eval`, the four
library tests, all 25 frames/75 prediction rows, and a check that hiding evaluation
ground truth leaves all selected predictions unchanged. Screenshots, labels, raw
scores and result pages remain local in ignored directories.

## Three-color co-occurrence experiment

`--ensemble-colors` adds color-gated versions of all three ensemble channels.
Each candidate window must contain at least two pixels in each HSV band before
it can compete in a gated channel. The window follows the scaled template size;
the check examines the original, unsmoothed screenshot. Prefix sums make window
counts constant-time. Candidate selection never uses reference boxes.

| Band | OpenCV hue (0–179) | Minimum saturation | Value range |
|---|---|---:|---|
| Brown | 8–30 | 40 | 20–220 |
| Red | 170–179 or 0–7 | 70 | 20–255 |
| Blue | 90–135 | 35 | 15–255 |

Parameters are in `TriColorConfig::default()` in `src/colors.rs`. These are
experimental color ranges, not universal definitions of the object's colors.
The existing three channels remain eligible: only 13 of 16 reference crops
contain all three bands. 151827, 171345 and 171419 have no qualifying blue pixels.
A mandatory gate on every channel would therefore discard real bobbers.

Each added channel uses its own maximum calibration-negative score plus 0.05,
with a minimum threshold of 0.60. Color co-occurrence alone never accepts a match.
The winning accepted channel still uses the largest score-minus-threshold margin.

```powershell
cargo run --locked --release --bin vision-eval -- samples/focus-expanded.csv output/colors-new.csv --ensemble-colors
.\scripts\summarize-ensemble.ps1 -CsvPath output/colors-new.csv -OutputPath output/colors-new.json -IncludeTriColor
cargo run --locked --release --bin color-profile -- samples/focus-expanded.csv output/colors-new.csv output/color-counts-new.csv
```

`color-profile` exports brown/red/blue pixel counts for reference and predicted
windows. Reference counts are diagnostics only. The new library tests cover
co-occurrence in the same window, red hue wraparound and rejection of dark noise.
The hardware bot and `image-test` are unchanged.

On the same development set this improves 10/13 to **11/13** correctly localized
positives, with **0/6** negative-frame false alarms. 171341 is recovered by
`red-trio`; 171419 and 171431 remain missed. All previously correct frames remain
correct. Added thresholds are 0.768771 (color-trio), 0.884498 (red-trio) and
0.798053 (hybrid-trio). This is not an independent test: these frames informed
development. Eight automated tests pass on native Windows/OpenCV.

## Additional cropped templates

The optional fourth argument loads extra crops alongside the existing views:

```powershell
cargo run --locked --release --bin vision-eval -- samples/focus-expanded.csv output/blob6-new.csv --ensemble-colors samples/extra-templates.csv
.\scripts\summarize-ensemble.ps1 -CsvPath output/blob6-new.csv -OutputPath output/blob6-new.json -IncludeTriColor
```

The extra-template CSV uses paths relative to its own directory:

```csv
file,source
blob6.png,WoWScrnShot_092526_171431.jpg
```

Add one row per crop. Every source must name a screenshot in the evaluation
manifest. Each crop is compared at 80%, 100% and 120% size using all selected
channels. The originating screenshot is always excluded for that crop, including
all scales and channels. Output records both `template` and `template_source`;
the summary also checks source exclusion. Extra templates are opt-in and do not
change the original bank or the hardware detector. Thresholds are recalibrated
on the same calibration negatives because extra views may also increase
background similarity. Image files and the local configuration remain ignored.

With the local `blob6.png` sourced from 171431, the labeled development set
remains at **11/13** correctly localized positives and **0/6** false alarms.
Neither 171419 nor 171431 is recovered. The extra view is never selected as
the best prediction on the other labeled frames; its best raw-channel candidate
is on negative frame 151803, below that channel's calibrated threshold. All
six thresholds stay unchanged. This result supports keeping the crop optional
until it can be checked on new, independently labeled screenshots.

This exclusion applies to evaluation only. The shared bot/`image-test` edge
detector loads `templates/blob6.png` as a regular production template and does
match its own 171431 source screenshot at threshold 0.80. Its source exclusion
in `vision-eval` remains necessary when judging generalization to other frames.
