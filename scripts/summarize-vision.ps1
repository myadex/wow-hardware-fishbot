param([Parameter(Mandatory)][string]$CsvPath)
$ErrorActionPreference = 'Stop'
$rows = Import-Csv -LiteralPath $CsvPath
if (!$rows) { throw 'Empty evaluation CSV.' }
$culture = [Globalization.CultureInfo]::InvariantCulture
$summary = foreach ($group in ($rows | Group-Object mode,region)) {
    $calibration = @($group.Group | Where-Object { $_.split -eq 'calibration' -and $_.present -eq '0' })
    $evaluation = @($group.Group | Where-Object split -eq 'evaluation')
    if (!$calibration.Count -or !$evaluation.Count) { throw 'Each group requires calibration negatives and evaluation rows.' }
    $maxNegative = ($calibration | ForEach-Object { [double]::Parse($_.score, $culture) } | Measure-Object -Maximum).Maximum
    $threshold = [Math]::Min(1.0, $maxNegative + 0.05)
    $correct = 0; $missed = 0; $wrongLocation = 0; $falseAlarm = 0; $correctNegative = 0
    foreach ($row in $evaluation) {
        $accepted = [double]::Parse($row.score, $culture) -ge $threshold
        if ($row.present -eq '0') {
            if ($accepted) { $falseAlarm++ } else { $correctNegative++ }
        } elseif (!$accepted) { $missed++
        } elseif ($row.center_in_target -eq 'true' -and [double]::Parse($row.iou, $culture) -ge 0.30) { $correct++
        } else { $wrongLocation++ }
    }
    [PSCustomObject]@{
        Mode=$group.Group[0].mode; Region=$group.Group[0].region
        Threshold=[Math]::Round($threshold,6); Correct=$correct; Missed=$missed
        WrongLocation=$wrongLocation; FalseAlarm=$falseAlarm; CorrectNegative=$correctNegative
    }
}
$summary
