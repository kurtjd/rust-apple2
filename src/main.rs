mod apple2;
use apple2::Apple2;
mod sound;
use sound::SoundHandler;

use std::{fs::File, io::Read};
use std::time::{Instant, Duration};

use sdl2::{EventPump, video::Window, render::Canvas, render::Texture};
use sdl2::pixels::PixelFormatEnum;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};

// Misc
const WIN_WIDTH: u32 = 280;
const WIN_HEIGHT: u32 = 192;
const PIXEL_SIZE: u32 = 3;
//const TEXT_ROWS: usize = 24;
const TEXT_COLS: usize = 40;
const PIXEL_ON_COLOR: u8 = 0xFF;
const PIXEL_OFF_COLOR: u8 = 0x00;
const CHAR_WIDTH: u32 = 7;
const CHAR_HEIGHT: u32 = 8;
const CHAR_ROM_SIZE: usize = 0x800;
const DISP_SCALE: u32 = 3;

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

fn handle_lores_gfx(apple2: &Apple2, gfx_helper: &mut GfxHelper) {
    let start_addrs = [0x400, 0x428, 0x450];
    let mut cell_idx = 0;

    for start in start_addrs {
        for j in 0..8 {
            for i in 0..40 {
                let idx = start + 0x80 * j + i;
                // Match over gfx mode
                // If text:
                print_character(apple2.cpu.ram[idx], cell_idx, gfx_helper);

                // If lores:


                cell_idx += 1;
            }
        }
    }
}

fn handle_gfx(apple2: &Apple2, gfx_helper: &mut GfxHelper) {
    // Match over gfx mode
    // If Hires:

    // If Lores/Text:
    handle_lores_gfx(apple2, gfx_helper);

    // Update canvas
    gfx_helper.pixel_surface.update(None, &gfx_helper.pixel_buf, (WIN_WIDTH * PIXEL_SIZE) as usize).unwrap();
    gfx_helper.canvas.copy(&gfx_helper.pixel_surface, None, None).unwrap();
    gfx_helper.canvas.present();
}

fn handle_input(apple2: &mut Apple2, event_pump: &mut EventPump) -> bool {
    for event in event_pump.poll_iter() {
        match event {
            Event::Quit {..} => {
                return false;
            }
            Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                apple2.cpu.reset();
            }
            Event::KeyDown { keycode: Some(keycode), keymod, .. } => {
                // Special case for arrow keys because they don't have an ASCII code
                if keycode == Keycode::Right {
                    apple2.input_char(0x95);
                    continue;
                } else if keycode == Keycode::Left {
                    apple2.input_char(0x88);
                    continue;
                }

                // Convert lowercase to uppercase
                let mut ascii = keycode as u8;
                if ascii >= b'a' && ascii <= b'z' {
                    ascii -= 32;
                }

                // Get the proper ASCII character if shift held
                if keymod.contains(Mod::LSHIFTMOD) || keymod.contains(Mod::RSHIFTMOD) {
                    ascii = Apple2::get_shift_ascii(ascii);
                }

                // Do nothing if not a valid Apple 2 key
                if !Apple2::is_valid_key(ascii) {
                    continue;
                }

                // Modify the value (if necessary) when CTRL is held
                if keymod.contains(Mod::LCTRLMOD) || keymod.contains(Mod::RCTRLMOD) {
                    ascii = Apple2::get_ctrl_ascii(ascii);
                }

                // The Apple 2 has the high bit set for ASCII characters
                apple2.input_char(ascii | (1 << 7));
            }
            _ => {}
        }
    }

    true
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

    let mut sound_handler = SoundHandler::new(&sdl_context);
    sound_handler.device.resume();

    let mut apple2 = Apple2::new();
    apple2.init();

    loop {
        handle_gfx(&apple2, &mut gfx_helper);
        if !handle_input(&mut apple2, &mut gfx_helper.event_pump) {
            break;
        }

        let start_time = Instant::now();

        let speaker_samples = apple2.run_frame(60, 44100);
        {
            //let mut lock = audio_device.lock();
            let mut lock = sound_handler.device.lock();
            for s in speaker_samples {
                lock.insert_sample(match s {
                    true => 0.5,
                    false => 0.0
                });
            }
        }

        let elapsed = start_time.elapsed().as_micros() as u64;
        let duration = Duration::from_micros(16667) - Duration::from_micros(elapsed);
        std::thread::sleep(duration);
    }
}