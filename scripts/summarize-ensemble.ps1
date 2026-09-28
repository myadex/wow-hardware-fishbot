param(
    [Parameter(Mandatory)][string]$CsvPath,
    [Parameter(Mandatory)][string]$OutputPath,
    [switch]$IncludeTriColor,
    [string]$LineCsvPath
)
$ErrorActionPreference = 'Stop'
$culture = [Globalization.CultureInfo]::InvariantCulture
$rows = @(Import-Csv -LiteralPath $CsvPath)
$modes = @('color-smooth', 'red-green', 'color-red')
if ($IncludeTriColor) { $modes += @('color-trio', 'red-trio', 'hybrid-trio') }
if ($LineCsvPath) {
    if (!$IncludeTriColor) { throw 'Line evidence requires -IncludeTriColor' }
    $lineRows = @(Import-Csv -LiteralPath $LineCsvPath)
    $lineScores = @{}
    foreach ($lineRow in $lineRows) {
        $key = "$($lineRow.file)|$($lineRow.kind)"
        if ($lineScores.ContainsKey($key)) { throw "Duplicate line evidence: $key" }
        $value = [double]::Parse($lineRow.score, $culture)
        if (![double]::IsFinite($value) -or $value -lt 0 -or $value -gt 1) { throw "Invalid line evidence: $key" }
        $lineScores[$key] = $lineRow
    }
    $extraRows = @(foreach ($row in $rows | Where-Object mode -eq 'color-trio') {
        $key = "$($row.file)|color-trio"
        if (!$lineScores.ContainsKey($key)) { throw "Missing line evidence: $key" }
        $evidence = $lineScores[$key]
        foreach ($coordinate in @('x', 'y', 'width', 'height')) {
            if ([int]$row.$coordinate -ne [int]$evidence.$coordinate) { throw "Mismatched line evidence: $key ($coordinate)" }
        }
        $copy = $row | Select-Object *
        $copy.mode = 'color-trio-line'
        $copy.score = ([double]::Parse($row.score, $culture) + 0.05 * [double]::Parse($evidence.score, $culture)).ToString('F6', $culture)
        $copy
    })
    $rows += $extraRows
    $modes += 'color-trio-line'
}
$thresholds = @{}
foreach ($mode in $modes) {
    $calibration = @($rows | Where-Object { $_.mode -eq $mode -and $_.split -eq 'calibration' -and $_.present -eq '0' })
    if (!$calibration.Count) { throw "Missing calibration negatives for $mode" }
    $scores = @($calibration | ForEach-Object { [double]::Parse($_.score, $culture) })
    # Never cap at 1: if no margin is available, this channel must reject all.
    $thresholds[$mode] = ($scores | Measure-Object -Maximum).Maximum + 0.05
    # A color gate alone is insufficient when calibration has no eligible windows.
    if ($mode.EndsWith('-trio') -or $mode -eq 'color-trio-line') { $thresholds[$mode] = [Math]::Max(0.60, $thresholds[$mode]) }
}
$predictions = @(foreach ($group in ($rows | Where-Object { $_.split -notin @('template', 'calibration') } | Group-Object file)) {
    if ($group.Count -ne $modes.Count -or @($group.Group.mode | Sort-Object -Unique).Count -ne $modes.Count) {
        throw "Expected one prediction per mode for $($group.Name)"
    }
    $accepted = @(foreach ($row in $group.Group) {
        if (!$thresholds.ContainsKey($row.mode)) { throw "Unexpected mode: $($row.mode)" }
        if ($row.template -eq $row.file -or $row.template_source -eq $row.file) { throw "Self-template leakage: $($row.file)" }
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
