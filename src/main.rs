mod apple2;
mod disk_controller;
mod dsk2woz;
mod graphics;
mod mem_manager;
mod sound;
mod wizard_of_woz;

use apple2::Apple2;
use mem_manager::MemManager;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use sdl2::event::Event;
use sdl2::keyboard::{Keycode, Mod};
use sdl2::EventPump;

const FRAME_RATE: u32 = 60;
const US_PER_FRAME: u64 = 1000000 / FRAME_RATE as u64;

fn handle_input(apple2: &mut Apple2, event_pump: &mut EventPump) -> bool {
    // TODO: Escape keys, and will need to change key for reset()

    for event in event_pump.poll_iter() {
        match event {
            Event::Quit { .. } => {
                return false;
            }
            Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => {
                apple2.reset();
            }
            Event::KeyDown {
                keycode: Some(keycode),
                keymod,
                ..
            } => {
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
                if ascii.is_ascii_lowercase() {
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
    /* Would be nice to move this all into the graphics module, but that requires making a
    self-referential data structure which is diffiult in Rust. Will revisit this in the future. */
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window(
            "Apple ][+",
            graphics::DISP_WIDTH * graphics::DISP_SCALE,
            graphics::DISP_HEIGHT * graphics::DISP_SCALE,
        )
        .position_centered()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    // Initialize Apple 2 emulator and insert disks
    /* Again, would like to move memory manager construction to apple2 but since the CPU also
    relies on the memory manager, this creates a self-referential data structure. In the future
    will have to find a better way to design what I'm trying to achieve to avoid this. */
    let mem_manager = Rc::new(RefCell::new(MemManager::new()));
    let mut apple2 = Apple2::new(&mem_manager, &sdl_context, &mut canvas, &texture_creator);
    apple2.init();

    if args.len() > 1 {
        let disk_file = &args[1];
        apple2.insert_disk(disk_file);
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
