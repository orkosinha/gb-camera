# Gameboy Camera Emulator

A specialized Game Boy emulator targeting [Game Boy Camera cartridges](https://gbdev.io/pandocs/Gameboy_Camera.html).

The emulator replicates the M64282FP image sensor and implements the full Game Boy hardware necessary to run the Game Boy Camera's ROM. The debug interface for this emulator is published [here](http://orkosinha.github.io/gb-camera/). A minimal ROM mocking functionality of the real cartridge is provided, but please load your own dump of the real cartridge.

## Building

This project uses [mise](https://mise.jdx.dev/) for task orchestration.

```bash
mise run dev       # Build and serve the web project
mise run test      # Run all tests
mise run check     # Lint with clippy
```
