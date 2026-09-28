# World of Warcraft Hardware Fishbot
Proof of conecpt Raspberry Pi fishing bot for World of Warcraft using HDMI capture and HID emulation, built with Rust. 


Find out more about this project on my blog: https://zuernerd.github.io/blog/2025/09/13/hardware-gamebot.html

## Development

For native Windows screenshot testing, see [the Windows guide](docs/windows.md).
The `image-test` command uses the bot's color-first, edge-fallback detection code
without capture or HID devices. The bundled color crops are in `color-templates/`.
For Pi 5 / C790 setup and a live image test without HID input, see
[the C790 guide](docs/pi5-c790.md). The bot and `capture-test` share startup
frame discard and configurable `FISHBOT_SWAP_RB` color correction.
The Pi bot casts with key `2`. When it detects a bite, it moves the USB HID
mouse to the bobber and right-clicks. Both keyboard (`/dev/hidg0`) and mouse
(`/dev/hidg1`) gadget functions are required. The pointer is parked at the
top-left before each cast so it does not cover the bobber during monitoring.

See [the Linux build guide](docs/building.md) for dependencies and building without
capture hardware. GitHub Actions compiles the project on Ubuntu 24.04; hardware
and image-recognition validation are separate steps.

## ⚠️ Disclaimer

**This project is for educational purposes only.** Using automation tools in World of Warcraft violates the terms of service and can result in account suspension or permanent bans. Use this code at your own risk. You will probably be banned!!


