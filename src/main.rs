#![allow(dead_code)]

use rust_6502::*;

use std::{fs::File, io::Read};
use std::thread;
use std::time::Duration;

use sdl2::{EventPump, video::Window, render::Canvas, render::Texture};
use sdl2::pixels::PixelFormatEnum;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};

// Addresses
const ROM_START_ADDR: usize = 0xD000;
const INPUT_DATA_ADDR: usize = 0xC000;

// Soft-switches (simply accessing these causes behavior)
const INPUT_CLEAR_ADDR: usize = 0xC010;
const SPEAKER_ADDR: usize = 0xC030;

const GFX_MODE_ADDR: usize = 0xC050;
const TXT_MODE_ADDR: usize = 0xC051;

const SINGLE_MODE_ADDR: usize = 0xC052;
const MIXED_MODE_ADDR: usize = 0xC053;

const PG1_MODE_ADDR: usize = 0xC054;
const PG2_MODE_ADDR: usize = 0xC055;

const LORES_MODE_ADDR: usize = 0xC056;
const HIRES_MODE_ADDR: usize = 0xC057;

// Misc
const WIN_WIDTH: u32 = 280;
const WIN_HEIGHT: u32 = 192;
const PIXEL_SIZE: u32 = 3;
const TEXT_ROWS: usize = 24;
const TEXT_COLS: usize = 40;
const PIXEL_ON_COLOR: u8 = 0xFF;
const PIXEL_OFF_COLOR: u8 = 0x00;
const CHAR_WIDTH: u32 = 7;
const CHAR_HEIGHT: u32 = 8;
const CHAR_ROM_SIZE: usize = 0x800;

struct GfxHelper<'a> {
    canvas: Canvas<Window>,
    event_pump: EventPump,
    pixel_buf: [u8; (WIN_WIDTH * WIN_HEIGHT * PIXEL_SIZE) as usize],
    pixel_surface: Texture<'a>,
    char_data: [u8; CHAR_ROM_SIZE]
}

fn print_character(val: u8, cell_idx: usize, gfx_helper: &mut GfxHelper) {
    let row = cell_idx / TEXT_COLS;
    let col = cell_idx % TEXT_COLS;

    // Mask off the upper two bits as they don't affect address
    let char_addr = ((val & 0x3F) as u32 * CHAR_HEIGHT) as usize;
    let mut pbuf_idx = row * (CHAR_HEIGHT * WIN_WIDTH * PIXEL_SIZE) as usize + col * (CHAR_WIDTH * PIXEL_SIZE) as usize;

    for i in char_addr..char_addr + CHAR_HEIGHT as usize {
        let mut char_map = gfx_helper.char_data[i];
        if val & (1 << 7) == 0 {
            char_map ^= 0xFF; // Invert all bits
        }
        char_map <<= 1;

        for _ in 0..CHAR_WIDTH {
            if char_map & (1 << 7) != 0 {
                gfx_helper.pixel_buf[pbuf_idx..pbuf_idx + PIXEL_SIZE as usize].fill(PIXEL_ON_COLOR);
            } else {
                gfx_helper.pixel_buf[pbuf_idx..pbuf_idx + PIXEL_SIZE as usize].fill(PIXEL_OFF_COLOR);
            }

            char_map <<= 1;
            pbuf_idx += PIXEL_SIZE as usize;
        }

        pbuf_idx -= (CHAR_WIDTH * PIXEL_SIZE) as usize;
        pbuf_idx += (WIN_WIDTH * PIXEL_SIZE) as usize;
    }
}

fn handle_lores_gfx(cpu: &Cpu6502, gfx_helper: &mut GfxHelper) {
    let start_addrs = [0x400, 0x428, 0x450];
    let mut cell_idx = 0;

    for start in start_addrs {
        for j in 0..8 {
            for i in 0..40 {
                let idx = start + 0x80 * j + i;
                // Match over gfx mode
                // If text:
                print_character(cpu.ram[idx], cell_idx, gfx_helper);

                // If lores:


                cell_idx += 1;
            }
        }
    }
}

fn handle_gfx(cpu: &Cpu6502, gfx_helper: &mut GfxHelper) {
    // Match over gfx mode
    // If Hires:

    // If Lores/Text:
    handle_lores_gfx(cpu, gfx_helper);

    // Update canvas
    gfx_helper.pixel_surface.update(None, &gfx_helper.pixel_buf, (WIN_WIDTH * PIXEL_SIZE) as usize).unwrap();
    gfx_helper.canvas.copy(&gfx_helper.pixel_surface, None, None).unwrap();
    gfx_helper.canvas.present();
}

fn handle_soft_sw(cpu: &mut Cpu6502) {
    for c in &mut cpu.cycles {
        match c.address {
            INPUT_CLEAR_ADDR => { cpu.ram[INPUT_DATA_ADDR] &= !(1 << 7) },
            // Other addresses
            _ => {}
        }
    }
}

fn handle_input(cpu: &mut Cpu6502, event_pump: &mut EventPump) -> bool {
    for event in event_pump.poll_iter() {
        match event {
            Event::Quit {..} => {
                return false;
            }
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                cpu.reset();
            }
            Event::KeyDown { keycode: Some(keycode), keymod, .. } => {
                if keycode == Keycode::LShift || keycode == Keycode::RShift {
                    continue;
                }

                let mut ascii = keycode as u8;
                if ascii >= b'a' && ascii <= b'z' {
                    ascii -= 32;
                }

                if keymod.contains(Mod::LSHIFTMOD) || keymod.contains(Mod::RSHIFTMOD) {
                    ascii = match ascii {
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
                    };
                }

                cpu.ram[INPUT_DATA_ADDR] = ascii | (1 << 7);
            }
            _ => {}
        }
    }

    true
}

fn load_rom(cpu: &mut Cpu6502) {
    let mut rom = File::open("roms/firmware/Apple2_Plus.rom").expect("Failed to opem ROM file!");
    rom.read_exact(&mut cpu.ram[ROM_START_ADDR..]).expect("Failed to read ROM data!");
}

fn main() {
    // SDL initialization
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    // Create a window
    let window = video_subsystem.window("Apple ][", WIN_WIDTH, WIN_HEIGHT)
        .position_centered()
        .build()
        .unwrap();

    // Get the canvas to draw on
    let canvas = window.into_canvas().build().unwrap();

    // Texture stuff
    let texture_creator = canvas.texture_creator();
    let texture = texture_creator.create_texture_static(PixelFormatEnum::RGB24, WIN_WIDTH, WIN_HEIGHT).unwrap();

    // Character data
    let mut char_rom = File::open("roms/firmware/charset.bin").expect("Failed to open charset ROM!");
    let mut char_array: [u8; CHAR_ROM_SIZE] = [0; CHAR_ROM_SIZE];
    char_rom.read_exact(&mut char_array).expect("Failed to read char ROM data!");

    let mut gfx_helper = GfxHelper {
        canvas: canvas,
        event_pump: sdl_context.event_pump().unwrap(),
        pixel_buf: [PIXEL_OFF_COLOR; (WIN_WIDTH * WIN_HEIGHT * PIXEL_SIZE) as usize],
        pixel_surface: texture,
        char_data: char_array
    };




    // Initialize CPU
    let mut cpu = Cpu6502::new(0xC000);
    load_rom(&mut cpu);
    cpu.reset();

    // Framerate limiting hack for now (~60Hz)
    // Apple II CPU freq: 1.24 MHz
    // Apple II cycles per frame: ~17,066
    // Miliseconds per frame: ~16
    let mut cpu_cycles: u32 = 0;
    loop {
        if cpu_cycles >= 17066 {
            handle_gfx(&cpu, &mut gfx_helper);
            if !handle_input(&mut cpu, &mut gfx_helper.event_pump) {
                break;
            }

            cpu_cycles = 0;
            thread::sleep(Duration::from_millis(16));
        }

        cpu.clear_cycles();
        cpu_cycles += cpu.tick() as u32;
        handle_soft_sw(&mut cpu);
    }
}
