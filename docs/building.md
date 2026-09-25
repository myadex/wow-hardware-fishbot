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
targets without starting the application or accessing a capture or HID device.

Passing this workflow does not demonstrate image-detection accuracy or Pi
compatibility. Image regression tests, formatting/lint checks and the Raspberry
Pi hardware acceptance run remain tracked in
[issue #7](https://github.com/myadex/wow-hardware-fishbot/issues/7).

The current application expects Linux V4L2 and `/dev/hidg*` devices at runtime.
Use the real Pi to validate HDMI input, USB reports, latency and power delivery.
An x86-64 CI executable cannot be run on the ARM64 Raspberry Pi; build on the Pi
for the hardware run. Raspberry Pi OS and its native dependency versions still
need to be validated on the actual device.
