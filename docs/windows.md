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
and the same game UI scale you intend to use on the Pi. The bot now searches
with resized, smoothed color crops at 0.8, 1.0 and 1.2 scale inside the central
screen area, then falls back to the original edge templates. Additional color
crops belong in `color-templates/`; keep a no-bobber sample set when changing
that bank.

```powershell
New-Item -ItemType Directory -Force samples, output
cargo run --locked --bin image-test -- samples\fishing.png output\fishing-result.png templates 0.80
```

The terminal prints the matching template, correlation score, and rectangle.
A green rectangle marks an accepted match. A score is not a probability.
The color threshold is 0.78; the optional command-line threshold controls only
the edge fallback (default 0.80). Both need checking on new capture conditions.
Existing output files are never overwritten; choose a new name for each run.
Exit codes are 0 for a match, 2 for no match, and 1 for an input/runtime error.

`templates/blob6.png` remains an edge template from screenshot 171431. The
new color bank includes six labeled screenshot views and three video views.
The video templates are tiny crops; the full recordings remain local. On the
local development set, after excluding each template's own source screenshot,
the combined detector localizes 8/9 other positive screenshots and rejects all
9 labeled no-bobber screenshots. It also localizes 18 reviewed video frames and
rejects two video frames before the bobber appears. These frames are from the
same inspected sessions, so they do not establish independent accuracy.

The offline command and Pi bot share the same detection code. The bot uses only
the keyboard HID gadget at `/dev/hidg0`: key `2` casts, and **F8** is tapped when
a bite is detected. Bind F8 in the game to the action you want on a bite. To
choose another bite key, set `FISHBOT_BITE_KEY` before starting the bot; accepted
names are A-Z, 0-9 and F1-F12. An invalid value stops the bot at startup.
The bot does not open `/dev/hidg1` or send mouse movement or clicks. If no
bobber is found, it waits briefly and casts again.

The screenshot tests do not validate bite detection over time. Two local
recordings were reviewed in [the bite-video report](bite-video-review.md),
which motivated the motion threshold and an inner 26×26 monitoring window for
32×32 video crops. Pi capture latency, keyboard
binding in the game, and recognition accuracy on actual gameplay still need
hardware validation.
The synthetic Rust tests verify basic matching behavior, not real-world accuracy.

Local smoke checks with the supplied `bobber1.png` gave correlation 1.0 for the
identical image and about 0.658 when pasted onto a black canvas. The latter is
correctly rejected at 0.80 and accepted at 0.50. Template border context affects
Canny edges; this is another reason to calibrate on actual screenshots instead
of assuming that one threshold is universally suitable.
