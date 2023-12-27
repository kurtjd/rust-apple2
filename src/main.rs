mod apple2;
mod sound;
mod graphics;
mod disk_controller;
mod wizard_of_woz;
mod dsk2woz;

use apple2::Apple2;

use std::time::{Instant, Duration};

use sdl2::EventPump;
use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};

const FRAME_RATE: u32 = 60;
const US_PER_FRAME: u64 = 1000000 / FRAME_RATE as u64;

fn handle_input(apple2: &mut Apple2, event_pump: &mut EventPump) -> bool {
    // TODO: Escape keys, and will need to change key for reset()

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
                    apple2.input_char(apple2::KEY_RIGHT);
                    continue;
                } else if keycode == Keycode::Left {
                    apple2.input_char(apple2::KEY_LEFT);
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

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Initialize SDL
    let sdl_context = sdl2::init().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    // Initialize video
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem.window(
        "Apple ][+",
        graphics::WIN_WIDTH * graphics::DISP_SCALE,
        graphics::WIN_HEIGHT * graphics::DISP_SCALE).position_centered().build().unwrap();
    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    // Initialize Apple 2 emulator and insert disks
    let mut apple2 = Apple2::new(&sdl_context, &mut canvas, &texture_creator);
    apple2.init();
    apple2.snd_handler.device.resume();

    if args.len() > 1 {
        apple2.insert_disk(&args[1]);
    }

    // Main loop
    loop {
        apple2.draw_frame(FRAME_RATE);
        if !handle_input(&mut apple2, &mut event_pump) {
            break;
        }

        let start_time = Instant::now();

        apple2.run_frame(FRAME_RATE);

        // Sleep for rest of frame period
        let elapsed = Duration::from_micros(start_time.elapsed().as_micros() as u64);
        let frame = Duration::from_micros(US_PER_FRAME);
        if frame > elapsed {
            let duration = frame - elapsed;
            std::thread::sleep(duration);
        }
    }
}