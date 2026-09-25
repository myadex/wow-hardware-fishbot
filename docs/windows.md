# Local image testing on Windows

This mode reads screenshots and writes annotated images. It does not open a camera,
connect to the Pi, or send mouse/keyboard input. Ubuntu and WSL are not required.

## Dependencies

- Rust MSVC via [rustup](https://rust-lang.org/tools/install/); the repository selects the toolchain.
- Visual Studio Build Tools, including Desktop development with C++ and a Windows SDK.
- [OpenCV 4.11.0 Windows build](https://github.com/opencv/opencv/releases/tag/4.11.0).
- [LLVM 20.1.8 Windows x64](https://github.com/llvm/llvm-project/releases/tag/llvmorg-20.1.8), including libclang.

The current PC has the official `opencv-4.11.0-windows.exe` and
`LLVM-20.1.8-win64.exe` archives extracted with 7-Zip under
`%USERPROFILE%\.fishbot-tools`. The expected files are
`opencv\build\x64\vc16\lib\opencv_world4110.lib` and `llvm20\bin\libclang.dll`.
On another PC, extract those same archives into this layout, or pass a different
root to the environment script. The script changes only the current process environment.

Open PowerShell in the repository:

```powershell
. .\scripts\windows-env.ps1
cargo test --locked --lib
cargo build --locked --bins
cargo run --locked --bin image-test -- --help
```

## Try a screenshot

Save a full-resolution PNG in a local `samples` directory. Start with 1920x1080
and the same game UI scale you intend to use on the Pi. Template matching does
not resize templates; the original templates may not match your UI scale or bobber.
Crop additional PNG templates tightly around your own bobber and put them in a
separate directory if necessary.

```powershell
New-Item -ItemType Directory -Force samples, output
cargo run --locked --bin image-test -- samples\fishing.png output\fishing-result.png templates 0.80
```

The terminal prints the matching template, correlation score, and rectangle.
A green rectangle marks an accepted match. A score is not a probability.
The initial threshold of 0.80 is uncalibrated: tune it with screenshots both with
and without a bobber, at different water backgrounds and lighting conditions.
Existing output files are never overwritten; choose a new name for each run.
Exit codes are 0 for a match, 2 for no match, and 1 for an input/runtime error.

The offline command and Pi bot share the same detection code. The bot now skips
mouse interaction when no match passes the threshold. The screenshot tests do
not validate splash detection over time, capture latency, mouse calibration,
or recognition accuracy on actual gameplay. Those require recordings or hardware.
The synthetic Rust tests verify basic matching behavior, not real-world accuracy.

Local smoke checks with the supplied `bobber1.png` gave correlation 1.0 for the
identical image and about 0.658 when pasted onto a black canvas. The latter is
correctly rejected at 0.80 and accepted at 0.50. Template border context affects
Canny edges; this is another reason to calibrate on actual screenshots instead
of assuming that one threshold is universally suitable.
