#![allow(dead_code)]

use rust_6502::*;
use std::{fs::File, io::Read};

mod settings {
    pub const CPU_CLK_SPEED: u32 = 1024000;
    pub const PERIPH_ROM_SZ: usize = 0x100;
}

mod address {
    pub const ROM_START: usize = 0xC000;
    pub const DISK2_START: usize = 0xC600;
    pub const FW_START: usize = 0xD000;
    pub const INPUT_DATA: usize = 0xC000;
}

mod soft_switch {
    pub const INPUT_CLEAR: usize = 0xC010; // Whole page
    pub const SPEAKER: usize = 0xC030; // Whole page
    pub const GFX_MODE: usize = 0xC050;
    pub const TXT_MODE: usize = 0xC051;
    pub const SINGLE_MODE: usize = 0xC052;
    pub const MIXED_MODE: usize = 0xC053;
    pub const PG1_MODE: usize = 0xC054;
    pub const PG2_MODE: usize = 0xC055;
    pub const LORES_MODE: usize = 0xC056;
    pub const HIRES_MODE: usize = 0xC057;
}

enum GfxMode {
    TEXT,
    LORES,
    HIRES
}

pub struct Apple2 {
    pub cpu: Cpu6502,
    gfx_mode: GfxMode,
    gfx_mixed_mode: bool,
    gfx_use_pg2: bool,
    speaker: bool
}

impl Apple2 {
    fn load_rom(&mut self) {
        // Firmware ROM
        let mut fw_rom = File::open(
            "roms/firmware/Apple2_Plus.rom"
        ).expect("Failed to opem firmware ROM!");

        fw_rom.read_exact(
            &mut self.cpu.ram[address::FW_START..]
        ).expect("Failed to read firmware ROM data!");

        // Disk II ROM
        let mut disc_rom = File::open(
            "roms/firmware/Disk2.rom"
        ).expect("Failed to open Disk II ROM!");

        disc_rom.read_exact(
            &mut self.cpu.ram[address::DISK2_START..address::DISK2_START + settings::PERIPH_ROM_SZ]
        ).expect("Failed to read Disk II ROM data!");
    }

    fn handle_soft_sw(&mut self) {
        for c in &mut self.cpu.cycles {
            match c.address {
                soft_switch::INPUT_CLEAR => {
                    self.cpu.ram[address::INPUT_DATA] &= !(1 << 7);
                },
                soft_switch::SPEAKER => {
                    self.speaker = !self.speaker;
                },
                soft_switch::GFX_MODE => {

                },
                soft_switch::TXT_MODE => {

                },
                soft_switch::SINGLE_MODE => {

                },
                soft_switch::MIXED_MODE => {

                },
                soft_switch::PG1_MODE => {

                },
                soft_switch::PG2_MODE => {

                },
                soft_switch::LORES_MODE => {

                },
                soft_switch::HIRES_MODE => {

                },
                _ => {}
            }
        }
    }

    pub fn new() -> Self {
        Apple2 {
            cpu: Cpu6502::new(address::ROM_START),
            gfx_mode: GfxMode::TEXT,
            gfx_mixed_mode: false,
            gfx_use_pg2: false,
            speaker: false
        }
    }

    pub fn init(&mut self) {
        self.load_rom();
        self.cpu.reset();
    }

    pub fn run_frame(&mut self, frame_rate: u32, sample_rate: u32) -> Vec<bool> {
        let mut frame_cycles = 0;
        let mut sample_cycles = 0;
        let mut speaker_samples: Vec<bool> = Vec::new();

        let cycles_per_frame = settings::CPU_CLK_SPEED / frame_rate;
        let cycles_per_sample = settings::CPU_CLK_SPEED / sample_rate;

        while frame_cycles < cycles_per_frame {
            let cycles = self.cpu.tick() as u32;
            frame_cycles += cycles;
            sample_cycles += cycles;

            if sample_cycles >= cycles_per_sample {
                speaker_samples.push(self.speaker);
                sample_cycles = 0;
            }

            self.handle_soft_sw();
        }

        speaker_samples
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
