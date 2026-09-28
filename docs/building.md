# Building without capture hardware

The baseline build environment is Ubuntu 24.04 on x86-64. Rust 1.98.1 is selected
by `rust-toolchain.toml`; `Cargo.lock` records the resolved Rust dependencies.
The native OpenCV and Clang libraries come from Ubuntu's package repositories.
These native package versions are not locked by Cargo, so this is a reproducible
Rust dependency resolution, not a bit-for-bit reproducible system image.

Install the Linux prerequisites:

```sh
sudo apt-get update
sudo apt-get install --yes --no-install-recommends build-essential pkg-config clang libclang-dev libopencv-dev
```

Install Rust using the [official rustup instructions](https://rust-lang.org/tools/install/).
Then, from the repository directory:

```sh
rustup show
pkg-config --modversion opencv4
cargo build --locked --all-targets
git diff --exit-code -- Cargo.lock
```

The first Rust command installs the pinned toolchain if it is not already present.
Run builds with `--locked` to reject unintended dependency changes. When changing
dependencies deliberately, update and commit `Cargo.lock` with the manifest change.

## Continuous integration

The Linux build workflow runs on pushes and pull requests. Its logs record the
Rust, Clang and native OpenCV versions. It compiles the application and test
targets and runs synthetic image-recognition tests without starting the hardware
application or accessing a capture or HID device.

Passing this workflow does not demonstrate image-detection accuracy or Pi
compatibility. Gameplay image regression tests, formatting/lint checks and the Raspberry
Pi hardware acceptance run remain tracked in
[issue #7](https://github.com/myadex/wow-hardware-fishbot/issues/7).

The current application expects Linux V4L2 and both gadget functions at
runtime: keyboard `/dev/hidg0` and relative mouse `/dev/hidg1`. The supplied
`scripts/start-hid-gadget.sh` creates both functions. Use the real Pi to
validate HDMI input, USB reports, pointer position, latency and power delivery.
For the C790 CSI pipeline and a live test without HID, see
[the Pi 5 guide](pi5-c790.md). The user's v4l2/FFmpeg check confirms 1080p60
input and full-frame BGR decoding; the shared Rust capture path still needs
live validation.
On a Pi 5, USB gadget/device mode is available on its **USB-C power port**, not
the USB-A ports. Raspberry Pi's [OTG guide](https://pip-assets.raspberrypi.com/categories/685-app-notes-guides-whitepapers/documents/RP-009276-WP-1-Using%20OTG%20mode%20on%20Raspberry%20Pi%20SBCs)
specifies `dtoverlay=dwc2,dr_mode=peripheral` in `/boot/firmware/config.txt`.
That USB-C port must connect to the game PC for the keyboard/mouse gadget.
The kit's 27 W USB-C power supply therefore cannot occupy that same port at
the same time; arrange adequate power separately before relying on the gadget.
PC USB power alone may be insufficient for a Pi 5 with an NVMe SSD and cooler.
An x86-64 CI executable cannot be run on the ARM64 Raspberry Pi; build on the Pi
for the hardware run. Raspberry Pi OS and its native dependency versions still
need to be validated on the actual device.
