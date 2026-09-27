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
