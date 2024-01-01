use rust_6502::*;
use crate::disk_controller::DiskController;
use crate::graphics::GraphicsHandler;
use crate::sound::SoundHandler;
use crate::mem_manager::MemManager;

use std::cell::RefCell;
use std::rc::Rc;
use std::{fs::File, io::Read};
use std::path::Path;

use sdl2::{Sdl, video::Window, video::WindowContext, render::Canvas, render::TextureCreator};

mod settings {
    pub const CPU_CLK_SPEED: u32 = 1024000;
    pub const PERIPH_ROM_SZ: usize = 0x100;
    pub const DISK_SLOT: usize = 0x60;
}

mod address {
    pub const DISK2_START: usize = 0xC600;
    pub const FW_START: usize = 0xD000;
    pub const INPUT_DATA: usize = 0xC000;
}

mod soft_switch {
    pub const INPUT_CLEAR: usize = 0xC010; // Whole page
}

pub struct Apple2<'a> {
    cpu: Cpu6502<'a>,
    mem_manager: &'a Rc<RefCell<MemManager>>,
    gfx_handler: GraphicsHandler<'a>,
    snd_handler: SoundHandler,
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
            &mut self.mem_manager.borrow_mut().memory[address::FW_START..]
        ).expect("Failed to read firmware ROM data!");

        // Disk II ROM
        let mut disc_rom = File::open(
            "roms/firmware/disk2.rom"
        ).expect("Failed to open Disk II ROM!");

        disc_rom.read_exact(
            &mut self.mem_manager.borrow_mut()
            .memory[address::DISK2_START..address::DISK2_START + settings::PERIPH_ROM_SZ]
        ).expect("Failed to read Disk II ROM data!");
    }

    fn handle_soft_sw(&mut self) {
        /* It would be nice to put this in the memory management module and have soft switches
        checked only when the CPU does a read/write, but would require the memory manager knowing
        about the Apple2 module which causes borrowing problems. I must learn more about lifetimes
        and borrowing rules for future programs... */
        let cycles = self.mem_manager.borrow().get_cycles();
        for c in &cycles {
            if c.address >= 0xC090 {
                self.disk_controller.handle_soft_sw(
                    c.address,
                    &mut self.mem_manager.borrow_mut().memory
                );
            } else if c.address >= 0xC080 {
                self.mem_manager.borrow_mut().handle_soft_sw(c.address);
            } else if c.address >= 0xC050 {
                self.gfx_handler.handle_soft_sw(c.address);
            } else if c.address >= 0xC030 {
                self.snd_handler.handle_soft_sw(c.address);
            } else if c.address == soft_switch::INPUT_CLEAR {
                self.mem_manager.borrow_mut().memory[address::INPUT_DATA] &= !(1 << 7);
            }
        }
    }

    pub fn new(
        mem_manager: &'a Rc<RefCell<MemManager>>,
        sdl_context: &Sdl,
        canvas: &'a mut Canvas<Window>,
        texture_creator: &'a TextureCreator<WindowContext>) -> Self {

        // Create closures for memory manager's read/write methods
        let mem_read = |address: usize| -> u8 {
            mem_manager.clone().borrow_mut().mem_read(address)
        };
        let mem_write = |address: usize, value: u8| {
            mem_manager.clone().borrow_mut().mem_write(address, value);
        }; 

        Apple2 {
            cpu: Cpu6502::new(
                Box::new(mem_read),
                Box::new(mem_write)
            ),
            mem_manager,
            gfx_handler: GraphicsHandler::new(canvas, texture_creator),
            snd_handler: SoundHandler::new(sdl_context),
            disk_controller: DiskController::new(settings::DISK_SLOT)
        }
    }

    pub fn init(&mut self) {
        self.load_rom();
        
        self.cpu.reset();
        self.snd_handler.device.resume();
    }

    pub fn reset(&mut self) {
        self.mem_manager.borrow_mut().reset();
        self.cpu.reset();
        self.disk_controller.reset();
    }

    pub fn insert_disk(&mut self, file_path: &String) {
        self.disk_controller.load_image(Path::new(file_path));
    } 

    pub fn run_frame(&mut self, frame_rate: u32) {
        let mut frame_cycles = 0;
        let cycles_per_frame = settings::CPU_CLK_SPEED / frame_rate;

        /* Sound stuff...
            Sound is hard okay? */
        let mut sample_cycles = 0;
        let mut speaker_samples: Vec<bool> = Vec::new();
        let mut polarity_change = false;
        let prev_polarity = self.snd_handler.polarity;
        let cycles_per_sample = settings::CPU_CLK_SPEED / crate::sound::SAMPLE_RATE;

        // Tick the CPU for this frame
        while frame_cycles < cycles_per_frame {
            let cycles = self.cpu.tick() as u32;
            frame_cycles += cycles;
            sample_cycles += cycles;

            if sample_cycles >= cycles_per_sample {
                speaker_samples.push(self.snd_handler.polarity);
                sample_cycles = 0;

                if self.snd_handler.polarity != prev_polarity {
                    polarity_change = true;
                }
            }

            self.handle_soft_sw();
            self.mem_manager.borrow_mut().clear_cycles();
        }

        /* Feed sound samples from this frame to the sound handler.
        If the polarity didn't change, don't insert samples so we don't get that buzzing that
        SDL produces for non-zero samples. */
        if polarity_change {
            self.snd_handler.insert_samples(&speaker_samples);
        }

        self.disk_controller.handle_motor_off_delay();
    }

    pub fn draw_frame(&mut self, frame_rate: u32) {
        self.gfx_handler.handle_gfx(frame_rate, &self.mem_manager.borrow().memory);
    }

    pub fn input_char(&mut self, ascii: u8) {
        self.mem_manager.borrow_mut().memory[address::INPUT_DATA] = ascii;
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