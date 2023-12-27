use rust_6502::*;
use crate::disk_controller::DiskController;
use crate::graphics::GraphicsHandler;
use crate::sound::SoundHandler;

use std::{fs::File, io::Read};
use std::path::Path;

use sdl2::{Sdl, video::Window, video::WindowContext, render::Canvas, render::TextureCreator};

mod settings {
    pub const CPU_CLK_SPEED: u32 = 1024000;
    pub const PERIPH_ROM_SZ: usize = 0x100;
    pub const DISK_SLOT: usize = 0x60;
}

mod address {
    pub const ROM_START: usize = 0xC000;
    pub const DISK2_START: usize = 0xC600;
    pub const FW_START: usize = 0xD000;
    pub const INPUT_DATA: usize = 0xC000;
}

mod soft_switch {
    pub const INPUT_CLEAR: usize = 0xC010; // Whole page
}

pub struct Apple2<'a> {
    pub cpu: Cpu6502,
    gfx_handler: GraphicsHandler<'a>,
    pub snd_handler: SoundHandler,
    disk_controller: DiskController
}

pub const KEY_RIGHT: u8 = 0x95;
pub const KEY_LEFT: u8 = 0x88;

impl <'a>Apple2<'a> {
    fn load_rom(&mut self) {
        // Firmware ROM
        let mut fw_rom = File::open(
            "roms/firmware/apple2_plus.rom"
        ).expect("Failed to open firmware ROM!");

        fw_rom.read_exact(
            &mut self.cpu.ram[address::FW_START..]
        ).expect("Failed to read firmware ROM data!");

        // Disk II ROM
        let mut disc_rom = File::open(
            "roms/firmware/disk2.rom"
        ).expect("Failed to open Disk II ROM!");

        disc_rom.read_exact(
            &mut self.cpu.ram[address::DISK2_START..address::DISK2_START + settings::PERIPH_ROM_SZ]
        ).expect("Failed to read Disk II ROM data!");
    }

    fn handle_soft_sw(&mut self) {
        for c in &self.cpu.cycles {
            if c.address >= 0xC080 {
                self.disk_controller.handle_soft_sw(c.address, &mut self.cpu.ram)
            } else if c.address >= 0xC050 {
                self.gfx_handler.handle_soft_sw(c.address);
            } else if c.address >= 0xC030 {
                self.snd_handler.handle_soft_sw(c.address);
            } else if c.address == soft_switch::INPUT_CLEAR {
                self.cpu.ram[address::INPUT_DATA] &= !(1 << 7);
            }
        }
    }

    pub fn new(
        sdl_context: &Sdl,
        canvas: &'a mut Canvas<Window>,
        texture_creator: &'a TextureCreator<WindowContext>) -> Self {
        Apple2 {
            cpu: Cpu6502::new(address::ROM_START),
            gfx_handler: GraphicsHandler::new(canvas, texture_creator),
            snd_handler: SoundHandler::new(sdl_context),
            disk_controller: DiskController::new(settings::DISK_SLOT)
        }
    }

    pub fn init(&mut self) {
        self.load_rom();
        self.cpu.reset();
    }

    pub fn insert_disk(&mut self, file_path: &String) {
        self.disk_controller.load_image(Path::new(file_path));
    } 

    pub fn run_frame(&mut self, frame_rate: u32) {
        let mut frame_cycles = 0;
        let mut sample_cycles = 0;
        let mut speaker_samples: Vec<bool> = Vec::new();

        let cycles_per_frame = settings::CPU_CLK_SPEED / frame_rate;
        let cycles_per_sample = settings::CPU_CLK_SPEED / crate::sound::SAMPLE_RATE;

        while frame_cycles < cycles_per_frame {
            let cycles = self.cpu.tick() as u32;
            frame_cycles += cycles;
            sample_cycles += cycles;

            if sample_cycles >= cycles_per_sample {
                speaker_samples.push(self.snd_handler.polarity);
                sample_cycles = 0;
            }

            self.handle_soft_sw();
        }

        self.disk_controller.handle_motor_off_delay();

        // Feed sound samples from this frame to the sound handler
        {
            let mut lock = self.snd_handler.device.lock();
            for s in speaker_samples {
                lock.insert_sample(match s {
                    true => crate::sound::SAMPLE_VOLUME,
                    false => 0.0
                });
            }
        }
    }

    pub fn draw_frame(&mut self, frame_rate: u32) {
        self.gfx_handler.handle_gfx(frame_rate, &self.cpu.ram);
    }

    pub fn input_char(&mut self, ascii: u8) {
        self.cpu.ram[address::INPUT_DATA] = ascii;
    }

    pub fn is_valid_key(ascii: u8) -> bool {
        // 8 = ASCII for backspace, 13 = ASCII for return/enter
        match ascii {
            b' '..=b'^' | b'_' | 8 | 13 => true,
            _ => false
        }
    }

    pub fn get_shift_ascii(ascii: u8) -> u8 {
        match ascii {
            b'1' => b'!',
            b'2' => b'@',
            b'3' => b'#',
            b'4' => b'$',
            b'5' => b'%',
            b'6' => b'^',
            b'7' => b'&',
            b'8' => b'*',
            b'9' => b'(',
            b'0' => b')',
            b'-' => b'_',
            b'=' => b'+',
            b'[' => b'{',
            b']' => b'}',
            b';' => b':',
            b'\'' => b'"',
            b',' => b'<',
            b'.' => b'>',
            b'/' => b'?',
            _ => ascii
        }
    }

    pub fn get_ctrl_ascii(ascii: u8) -> u8 {
        // Ctrl only modified A-Z keys by clearing the 6th bit
        match ascii >= b'A' && ascii <= b'Z' {
            true => ascii & !(1 << 6),
            false => ascii
        }
    }
}