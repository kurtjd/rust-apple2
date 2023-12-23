use sdl2::pixels::PixelFormatEnum;
use sdl2::render::TextureCreator;
use sdl2::video::WindowContext;
use sdl2::{video::Window, render::Canvas, render::Texture};

use std::{fs::File, io::Read};

pub const WIN_WIDTH: u32 = 280;
pub const WIN_HEIGHT: u32 = 192;
pub const DISP_SCALE: u32 = 3;

const PIXEL_SIZE: u32 = 3;
//const TEXT_ROWS: usize = 24;
const TEXT_COLS: usize = 40;
const PIXEL_ON_COLOR: u32 = 0x00009900;
const PIXEL_OFF_COLOR: u32 = 0x00000000;
const CHAR_WIDTH: u32 = 7;
const CHAR_HEIGHT: u32 = 8;
const CHAR_ROM_SIZE: usize = 0x800;
const FLASH_RATE: u32 = 4;

pub struct GraphicsHandler<'a> {
    canvas: &'a mut Canvas<Window>,
    pixel_buf: [u8; (WIN_WIDTH * WIN_HEIGHT * PIXEL_SIZE) as usize],
    pixel_surface: Texture<'a>,
    char_data: [u8; CHAR_ROM_SIZE],
    frame_count: u32,
    invert_text: bool
}

impl <'a> GraphicsHandler <'a> {
    fn load_char_set() -> [u8; CHAR_ROM_SIZE] {
        let mut char_rom = File::open(
            "roms/firmware/Character_Set.rom"
        ).expect("Failed to open charset ROM!");

        let mut char_array: [u8; CHAR_ROM_SIZE] = [0; CHAR_ROM_SIZE];
        char_rom.read_exact(&mut char_array).expect("Failed to read char ROM data!");
        char_array
    }

    fn print_character(&mut self, val: u8, cell_idx: usize) {
        let row = cell_idx / TEXT_COLS;
        let col = cell_idx % TEXT_COLS;

        // Mask off the upper two bits as they don't affect address
        // Then multiply by 8 (since each character is represented by 8 bytes)
        let char_addr = ((val & 0x3F) as u32 * CHAR_HEIGHT) as usize;

        // Convert row and col into an index into the 1D pixel buffer
        let mut pbuf_idx = row * (CHAR_HEIGHT * WIN_WIDTH * PIXEL_SIZE) as usize
                           + col * (CHAR_WIDTH * PIXEL_SIZE) as usize;


        // For every byte (row) in character map
        for i in char_addr..char_addr + CHAR_HEIGHT as usize {
            let mut char_map = self.char_data[i];

            // 7th bit tells us if in invert mode
            // 6th bit tells us if in flash mode
            // So invert bits if in ivert mode, or in flash mode and invert_text is true
            if (val & (1 << 7) == 0) && (val & (1 << 6) == 0 || self.invert_text) {
                char_map ^= 0xFF; // Invert all bits
            }
            char_map <<= 1; // Then shift off high bit because we don't need it

            // For every dot in row of character map
            for _ in 0..CHAR_WIDTH {
                let pixel_color = match char_map & (1 << 7) != 0 {
                    true => PIXEL_ON_COLOR,
                    false => PIXEL_OFF_COLOR
                };
                self.pixel_buf[pbuf_idx] =     ((pixel_color & 0x00FF0000) >> 16) as u8;
                self.pixel_buf[pbuf_idx + 1] = ((pixel_color & 0x0000FF00) >>  8) as u8;
                self.pixel_buf[pbuf_idx + 2] = ((pixel_color & 0x000000FF) >>  0) as u8;

                char_map <<= 1;
                pbuf_idx += PIXEL_SIZE as usize;
            }

            // Set pixel buffer index to next character row down
            pbuf_idx -= (CHAR_WIDTH * PIXEL_SIZE) as usize;
            pbuf_idx += (WIN_WIDTH * PIXEL_SIZE) as usize;
        }
    }

    fn handle_lores_gfx(&mut self, buffer: &[u8]) {
        let start_addrs = [0x400, 0x428, 0x450];
        let mut cell_idx = 0;

        for start in start_addrs {
            for j in 0..8 {
                for i in 0..40 {
                    let idx = start + 0x80 * j + i;
                    // Match over gfx mode
                    // If text:
                    self.print_character(buffer[idx], cell_idx);

                    // If lores:


                    cell_idx += 1;
                }
            }
        }
    }

    pub fn handle_gfx(&mut self, frame_rate: u32, buffer: &[u8]) {
        // Match over gfx mode
        // If Hires:

        // If Lores/Text:
        self.handle_lores_gfx(buffer);

        // Update canvas
        self.pixel_surface.update(
            None,
            &self.pixel_buf, 
            (WIN_WIDTH * PIXEL_SIZE) as usize).unwrap();
        self.canvas.copy(&self.pixel_surface, None, None).unwrap();
        self.canvas.present();

        // Keep track when to "flash"
        self.frame_count += 1;
        if self.frame_count >= frame_rate / FLASH_RATE {
            self.invert_text = !self.invert_text;
            self.frame_count = 0;
        }
    }

    pub fn new(
        canvas: &'a mut Canvas<Window>,
        texture_creator: &'a TextureCreator<WindowContext>) -> Self {
        GraphicsHandler {
            canvas,
            pixel_buf: [0; (WIN_WIDTH * WIN_HEIGHT * PIXEL_SIZE) as usize],
            pixel_surface: texture_creator.create_texture_static(
                PixelFormatEnum::RGB24,
                WIN_WIDTH,
                WIN_HEIGHT).unwrap(),
            char_data: GraphicsHandler::load_char_set(),
            frame_count: 0,
            invert_text: false
        }
    }
}