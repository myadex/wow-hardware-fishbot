param(
    [Parameter(Mandatory)][string]$CsvPath,
    [Parameter(Mandatory)][string]$OutputPath
)
$ErrorActionPreference = 'Stop'
$culture = [Globalization.CultureInfo]::InvariantCulture
$rows = @(Import-Csv -LiteralPath $CsvPath)
$modes = @('color-smooth', 'red-green', 'color-red')
$thresholds = @{}
foreach ($mode in $modes) {
    $calibration = @($rows | Where-Object { $_.mode -eq $mode -and $_.split -eq 'calibration' -and $_.present -eq '0' })
    if (!$calibration.Count) { throw "Missing calibration negatives for $mode" }
    $scores = @($calibration | ForEach-Object { [double]::Parse($_.score, $culture) })
    # Never cap at 1: if no margin is available, this channel must reject all.
    $thresholds[$mode] = ($scores | Measure-Object -Maximum).Maximum + 0.05
}
$predictions = @(foreach ($group in ($rows | Where-Object { $_.split -notin @('template', 'calibration') } | Group-Object file)) {
    if ($group.Count -ne $modes.Count -or @($group.Group.mode | Sort-Object -Unique).Count -ne $modes.Count) {
        throw "Expected one prediction per mode for $($group.Name)"
    }
    $accepted = @(foreach ($row in $group.Group) {
        if (!$thresholds.ContainsKey($row.mode)) { throw "Unexpected mode: $($row.mode)" }
        if ($row.template -eq $row.file) { throw "Self-template leakage: $($row.file)" }
        $score = [double]::Parse($row.score, $culture)
        if ($score -ge $thresholds[$row.mode]) {
            [PSCustomObject]@{ Row=$row; Margin=$score-$thresholds[$row.mode] }
        }
    })
    # Selection depends only on scores, never presence labels or target rectangles.
    $selected = $accepted | Sort-Object -Property @{Expression='Margin';Descending=$true},@{Expression={$_.Row.mode};Descending=$false} | Select-Object -First 1
    $row = $group.Group[0]
    $outcome = 'unknown'
    if ($row.present -eq '0') { $outcome = if ($selected) { 'false_alarm' } else { 'correct_negative' }
    } elseif ($row.present -eq '1') {
        if (!$selected) { $outcome = 'missed'
        } elseif ($selected.Row.center_in_target -eq 'true' -and [double]::Parse($selected.Row.iou, $culture) -ge 0.30) { $outcome = 'correct'
        } else { $outcome = 'wrong_location' }
    }
    [PSCustomObject]@{
        File=$group.Name; Split=$row.split; Present=$row.present; Accepted=[bool]$selected
        Outcome=$outcome; Mode=$(if ($selected) { $selected.Row.mode } else { $null })
        Score=$(if ($selected) { [double]::Parse($selected.Row.score,$culture) } else { $null })
        Box=$(if ($selected) { @{x=[int]$selected.Row.x;y=[int]$selected.Row.y;width=[int]$selected.Row.width;height=[int]$selected.Row.height} } else { $null })
    }
})
$result = [ordered]@{
    Thresholds=$thresholds
    Selection='Accepted mode with largest score minus its calibration threshold; ties sorted by mode name'
    IndependentTest=$false
    Counts=@($predictions | Group-Object Outcome | Select-Object Name,Count)
    Predictions=$predictions
}
$json = $result | ConvertTo-Json -Depth 8
$stream = [IO.File]::Open($OutputPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
try {
    $writer = [IO.StreamWriter]::new($stream, [Text.UTF8Encoding]::new($false))
    try { $writer.Write($json) } finally { $writer.Dispose() }
} finally { $stream.Dispose() }
$result.Counts
