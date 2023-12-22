#![allow(dead_code)]

use rust_6502::*;

use std::{fs::File, io::Read};
use std::time::{Instant, Duration};

use sdl2::audio::{AudioCallback, AudioSpecDesired};
use sdl2::{EventPump, video::Window, render::Canvas, render::Texture};
use sdl2::pixels::PixelFormatEnum;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};

// Addresses
const ROM_START_ADDR: usize = 0xC000;
const DISK2_START_ADDR: usize = 0xC600;
const FW_START_ADDR: usize = 0xD000;
const INPUT_DATA_ADDR: usize = 0xC000;

// Soft-switches (simply accessing these causes behavior)

// Note: Accessing any value in the page (for keyboard/speaker) causes behavior
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
const PERIPH_ROM_SZ: usize = 0x100;
const DISP_SCALE: u32 = 3;


const BUF_SZ: usize = 1024;

struct SquareWave {
    buffer: [f32; BUF_SZ],
    sample_idx: usize,
    buf_idx: usize
}

impl SquareWave {
    fn insert_sample(&mut self, sample: f32) {
        self.buffer[self.buf_idx] = sample;

        self.buf_idx += 1;
        if self.buf_idx >= BUF_SZ {
            self.buf_idx = 0;
        }
    }
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        for x in out.iter_mut() {
            if self.sample_idx == self.buf_idx {
                *x = 0.0;
                return;
            }

            *x = self.buffer[self.sample_idx];

            self.sample_idx += 1;
            if self.sample_idx >= BUF_SZ {
                self.sample_idx = 0;
            }
        }
    }
}



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
    // Then multiply by 8 (since each character is represented by 8 bytes)
    let char_addr = ((val & 0x3F) as u32 * CHAR_HEIGHT) as usize;

    // Convert row and col into an index into the 1D pixel buffer
    let mut pbuf_idx = row * (CHAR_HEIGHT * WIN_WIDTH * PIXEL_SIZE) as usize + col * (CHAR_WIDTH * PIXEL_SIZE) as usize;

    // For every byte (row) in character map
    for i in char_addr..char_addr + CHAR_HEIGHT as usize {
        let mut char_map = gfx_helper.char_data[i];

        // If MSB is low in ASCII value, invert colors
        if val & (1 << 7) == 0 {
            char_map ^= 0xFF; // Invert all bits
        }
        char_map <<= 1; // Then shift off high bit because we don't need it

        // For every dot in row of character map
        for _ in 0..CHAR_WIDTH {
            // Draw the correct color depending on if bit is high or not
            if char_map & (1 << 7) != 0 {
                gfx_helper.pixel_buf[pbuf_idx..pbuf_idx + PIXEL_SIZE as usize].fill(PIXEL_ON_COLOR);
            } else {
                gfx_helper.pixel_buf[pbuf_idx..pbuf_idx + PIXEL_SIZE as usize].fill(PIXEL_OFF_COLOR);
            }

            char_map <<= 1;
            pbuf_idx += PIXEL_SIZE as usize;
        }

        // Set pixel buffer index to next character row down
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

fn handle_soft_sw(cpu: &mut Cpu6502, speaker: &mut bool) {
    for c in &mut cpu.cycles {
        match c.address {
            INPUT_CLEAR_ADDR => { cpu.ram[INPUT_DATA_ADDR] &= !(1 << 7) },
            SPEAKER_ADDR => { *speaker = !*speaker }
            // Other addresses
            _ => {}
        }
    }
}

fn is_valid_key(ascii: u8) -> bool {
    // 8 = ASCII for backspace, 13 = ASCII for return/enter
    match ascii {
        b' '..=b'^' | b'_' | 8 | 13 => true,
        _ => false
    }
}

fn get_shift_ascii(ascii: u8) -> u8 {
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

fn get_ctrl_ascii(ascii: u8) -> u8 {
    // Ctrl only modified A-Z keys by clearing the 6th bit
    match ascii >= b'A' && ascii <= b'Z' {
        true => ascii & !(1 << 6),
        false => ascii
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
                // Special case for arrow keys because they don't have an ASCII code
                if keycode == Keycode::Right {
                    cpu.ram[INPUT_DATA_ADDR] = 0x95;
                    continue;
                } else if keycode == Keycode::Left {
                    cpu.ram[INPUT_DATA_ADDR] = 0x88;
                    continue;
                }

                // Convert lowercase to uppercase
                let mut ascii = keycode as u8;
                if ascii >= b'a' && ascii <= b'z' {
                    ascii -= 32;
                }

                // Get the proper ASCII character if shift held
                if keymod.contains(Mod::LSHIFTMOD) || keymod.contains(Mod::RSHIFTMOD) {
                    ascii = get_shift_ascii(ascii);
                }

                // Do nothing if not a valid Apple 2 key
                if !is_valid_key(ascii) {
                    continue;
                }

                // Modify the value (if necessary) when CTRL is held
                if keymod.contains(Mod::LCTRLMOD) || keymod.contains(Mod::RCTRLMOD) {
                    ascii = get_ctrl_ascii(ascii);
                }

                // The Apple 2 has the high bit set for ASCII characters
                cpu.ram[INPUT_DATA_ADDR] = ascii | (1 << 7);
            }
            _ => {}
        }
    }

    true
}

fn load_rom(cpu: &mut Cpu6502) {
    // Firmware ROM
    let mut fw_rom = File::open("roms/firmware/Apple2_Plus.rom").expect("Failed to opem firmware ROM!");
    fw_rom.read_exact(&mut cpu.ram[FW_START_ADDR..]).expect("Failed to read firmware ROM data!");

    // Disk II ROM
    let mut disc_rom = File::open("roms/firmware/Disk2.rom").expect("Failed to open Disk II ROM!");
    disc_rom.read_exact(&mut cpu.ram[DISK2_START_ADDR..DISK2_START_ADDR + PERIPH_ROM_SZ]).expect("Failed to read Disk II ROM data!");
}

fn load_char_set() -> [u8; CHAR_ROM_SIZE] {
    let mut char_rom = File::open("roms/firmware/Character_Set.rom").expect("Failed to open charset ROM!");
    let mut char_array: [u8; CHAR_ROM_SIZE] = [0; CHAR_ROM_SIZE];
    char_rom.read_exact(&mut char_array).expect("Failed to read char ROM data!");
    char_array
}

fn main() {
    // SDL initialization
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    // Create a window
    let window = video_subsystem.window("Apple ][", WIN_WIDTH * DISP_SCALE, WIN_HEIGHT * DISP_SCALE)
        .position_centered()
        .build()
        .unwrap();

    // Get the canvas to draw on
    let canvas = window.into_canvas().build().unwrap();

    // Create a texture that we use as a pixel map to draw to
    let texture_creator = canvas.texture_creator();
    let texture = texture_creator.create_texture_static(PixelFormatEnum::RGB24, WIN_WIDTH, WIN_HEIGHT).unwrap();

    // Create helper struct that gives us quick access to SDL graphical and event stuff
    let mut gfx_helper = GfxHelper {
        canvas: canvas,
        event_pump: sdl_context.event_pump().unwrap(),
        pixel_buf: [PIXEL_OFF_COLOR; (WIN_WIDTH * WIN_HEIGHT * PIXEL_SIZE) as usize],
        pixel_surface: texture,
        char_data: load_char_set()
    };
    
    let audio_subsystem = sdl_context.audio().unwrap();

    let audio_spec = AudioSpecDesired {
        freq: Some(44100),
        channels: Some(1),
        samples: Some(512)
    };

    let wave = SquareWave {
        buffer: [0.0; BUF_SZ],
        sample_idx: 0,
        buf_idx: 0
    };

    let mut audio_device = audio_subsystem.open_playback(None, &audio_spec, |_| { wave }).unwrap();
    audio_device.resume();

    // Initialize CPU and load firmware/ROMs
    let mut cpu = Cpu6502::new(ROM_START_ADDR);
    load_rom(&mut cpu);
    cpu.reset();

    // Video Framerate: ~60Hz
    // Apple II CPU freq: ~1024 kHz
    // Apple II CPU cycles per frame: ~17,066.67
    // Miliseconds per frame: ~16.667
    // Speaker sample rate: ~44.1 kHz
    // Apple II CPU cycles per sample: ~23
    // Samples per frame: 735
    // Microseconds per sample: ~22.6
    let mut frame_cycles: u32 = 0;
    let mut speaker = false;
    let mut spk_cycles: u32 = 0;

    loop {
        handle_gfx(&cpu, &mut gfx_helper);
        if !handle_input(&mut cpu, &mut gfx_helper.event_pump) {
            break;
        }

        let start_time = Instant::now();
        while frame_cycles < 17067 {
            let cycles = cpu.tick() as u32;
            frame_cycles += cycles;
            spk_cycles += cycles;

            if spk_cycles >= 23 {
                let mut lock = audio_device.lock();
                lock.insert_sample(match speaker {
                    true => 0.5,
                    false => 0.0
                });
                spk_cycles = 0;
            }

            handle_soft_sw(&mut cpu, &mut speaker);
        }
        frame_cycles = 0;

        let elapsed = start_time.elapsed().as_micros() as u64;
        let duration = Duration::from_micros(16667) - Duration::from_micros(elapsed);
        
        std::thread::sleep(duration);
    }
}
