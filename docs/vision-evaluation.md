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
calibration-negative score plus 0.05, capped at 1.0. A correct localization
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
