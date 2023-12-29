# Apple II+ Emulator
|Aztec|Applesoft BASIC|
|----|------------------------------|
|<img src = "/screenshots/aztec.png?raw=true">|<img src = "/screenshots/basic.png?raw=true">|

|Lode Runner|Castle Wolfenstein|
|-------------|----|
|<img src = "/screenshots/lode_runner.png?raw=true">|<img src = "/screenshots/castle_wolfenstein?raw=true">|

|Cannonball Blitz|DOS 3.3|
|--------------|---------------|
|<img src = "/screenshots/cannonball_blitz.gif?raw=true">|<img src = "/screenshots/dos.png?raw=true">|

Just another Apple II+ emulator I've been writing to get more familiar with Rust and to indulge my interest in emulating retro machines.
This is my first attempt at a Rust program so the code can likely definitely use a lot of improvement, but I felt like I've already improved my Rust skills a lot so still happy with the outcome!

As the title states, this emulates your typical Apple II+ with a display, keyboard, 1-bit speaker, floppy disk drive, and 48k RAM.
Thus, software written for later versions of the Apple II such as the Apple IIe will not work correctly.
In addition, software making use of various peripherals not listed will likely not work correctly as well.
However the base Apple II+ was still a powerful machine for its time so plenty of fun to be had with this emulator!

## Features
### MOS 6502 CPU
Makes use of my [6502 emulator](https://github.com/kurtjd/rust-6502) I wrote for this project, which runs the CPU at roughly 1.24 MHz.
The CPU passes a wide gamut of tests and is "cycle accurate" in the sense that every opcode when executed accesses the address and data busses in the correct order
(which is necessary for the Apple II, which relies on the CPU accessing various memory addresses to toggle various "soft-switches" which control hardware).

### Display
Supports text, lores, hires, and mixed mode graphics.
Though hires graphics are quite tricky in that certain color combinations would produce "fringing" and other artifacts on some displays of the time.
I've managed to reproduce this "fringing" effect somewhat, though getting it perfect would be quite involved. However, it's pretty close!

### Keyboard
Supports the typical keys from the Apple II keyboard of the time, though I have to still implement the reset keys.

### Sound
Although the Apple II+ only had a simple 1-bit speaker, which could generate tones of various frequences by changing the polarity of the speaker, this can actually be a bit tricky to emulate on modern audio devices.
For my purposes, I simply sample the polarity of the speaker at a rate of 44.1 kHz and feed those samples as a square wave to the audio output, which seems to work well enough though of course this can definitely be improved to get further sound fidelity.

### Floppy Disk Controller
The disc controller is quite a deep rabbit hole, as the Apple II was designed in such a way that disk drives did not have their own CPU to control the disk motor as other drives of the time did,
thus all programs would have to manually control the disk drive themselves (though usually with the help of a DOS).

Because of this, a lot of software at the time would exploit the quirks of hardware for things like copy-protection purposes.
Additionally, due to the limitations of storage media at the time, data stored on disks does not resemble the data that would actually end up in RAM. Among other things, data had to go through "6 and 2" encoding to be converted to valid disk bytes.

Having said all that, perfect emulation of the disk controller can be quite an endeavor depending on how deep you want to go to support some software relying on the more obscure hardware quirks.

My emulator can support DSK disk images (simply the bytes of each track and sector on a disk as they would end up in RAM) as well as certain WOZ disk images (which contain the raw data as it would actually be stored on disk),
though I plan to improve this a bit more as it seems certain disks don't boot correctly, which means I may have some inaccuracies in my emulation.

### RAM
Supports 48k of RAM, and an additional 12k of ROM. Fortunately the Apple II+ didn't utilize bank switching, so didn't have to worry about that!


## Build
Simply run `cargo build` and all dependencies will be automagically included and built. Thanks Cargo!

## Run
To run without a disk inserted (which will provide the Applesoft BASIC interpreter and system monitor):  
`cargo run`

To run with a disk:  
`cargo run <PATH-TO-DISK-IMAGE>`

## Usage
After starting, if a disk image is inserted the Apple II firmware will automatically boot the disk after a short period. If a disk is not inserted, press the `Esc` key to reset the CPU and enter the Applesoft BASIC prompt. The `Esc` key can be used to reset the CPU at anytime.

## TODO
* Improve disk controller and add disk write support (as well as support for 2nd disk drive)
* Implement joystick emulation
* Make some adjustments to keyboard emulation
* Perform additional refactoring and cleanup

## License
This project is licensed under the MIT license and is completely free to use and modify.
