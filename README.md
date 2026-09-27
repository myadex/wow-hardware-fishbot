# World of Warcraft Hardware Fishbot
Proof of conecpt Raspberry Pi fishing bot for World of Warcraft using HDMI capture and HID emulation, built with Rust. 


Find out more about this project on my blog: https://zuernerd.github.io/blog/2025/09/13/hardware-gamebot.html

## Development

For native Windows screenshot testing, see [the Windows guide](docs/windows.md).
The `image-test` command uses the bot's color-first, edge-fallback detection code
without capture or HID devices. The bundled color crops are in `color-templates/`.
The Pi bot casts with key `2` and taps **F8** when a bite is detected. Bind F8
to your bite action in the game, or set `FISHBOT_BITE_KEY` to another key
(A-Z, 0-9, F1-F12). No mouse HID device is needed.

See [the Linux build guide](docs/building.md) for dependencies and building without
capture hardware. GitHub Actions compiles the project on Ubuntu 24.04; hardware
and image-recognition validation are separate steps.

## ⚠️ Disclaimer

**This project is for educational purposes only.** Using automation tools in World of Warcraft violates the terms of service and can result in account suspension or permanent bans. Use this code at your own risk. You will probably be banned!!


