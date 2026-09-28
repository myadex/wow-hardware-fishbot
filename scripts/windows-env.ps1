# Dot-source from PowerShell: . .\scripts\windows-env.ps1
param([string]$ToolRoot = "$env:USERPROFILE\.fishbot-tools")
$ErrorActionPreference = 'Stop'
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (!(Test-Path -LiteralPath $vswhere)) { throw 'Install Visual Studio Build Tools with Desktop development with C++ first.' }
$vsRoot = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$vsRoot) { throw 'Visual Studio C++ tools were not found.' }
$devCmd = Join-Path $vsRoot 'Common7\Tools\VsDevCmd.bat'
$envLines = & $env:COMSPEC /d /c "call `"$devCmd`" -arch=x64 -host_arch=x64 >nul && set"
if ($LASTEXITCODE -ne 0) { throw 'Could not initialize the MSVC environment.' }
foreach ($line in $envLines) {
    if ($line -match '^([^=]+)=(.*)$') { [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process') }
}
$env:OPENCV_INCLUDE_PATHS = Join-Path $ToolRoot 'opencv\build\include'
$env:OPENCV_LINK_PATHS = Join-Path $ToolRoot 'opencv\build\x64\vc16\lib'
$env:OPENCV_LINK_LIBS = 'opencv_world4110'
$env:LIBCLANG_PATH = Join-Path $ToolRoot 'llvm20\bin'
foreach ($required in @("$env:LIBCLANG_PATH\libclang.dll", "$env:OPENCV_LINK_PATHS\opencv_world4110.lib", "$env:OPENCV_INCLUDE_PATHS\opencv2\core.hpp")) {
    if (!(Test-Path -LiteralPath $required)) { throw "Missing native dependency: $required. See docs/windows.md." }
}
$env:PATH = "$env:USERPROFILE\.cargo\bin;$ToolRoot\opencv\build\x64\vc16\bin;$env:LIBCLANG_PATH;$env:PATH"
Write-Host 'Rust / OpenCV 4.11 / LLVM 20 / MSVC x64 ready for this PowerShell session.'
