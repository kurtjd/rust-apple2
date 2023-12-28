use sdl2::pixels::PixelFormatEnum;
use sdl2::render::TextureCreator;
use sdl2::video::WindowContext;
use sdl2::{video::Window, render::Canvas, render::Texture};

use std::{fs::File, io::Read};

pub const WIN_WIDTH: u32 = 280;
pub const WIN_HEIGHT: u32 = 192;
pub const DISP_SCALE: u32 = 3;

const PIXEL_SIZE: u32 = 3;
const CELL_ROWS: usize = 24;
const CELL_COLS: usize = 40;
const PIXEL_ON_COLOR: u32 = 0x00FFFFFF;
const PIXEL_OFF_COLOR: u32 = 0x00000000;
const CELL_WIDTH: u32 = 7;
const CELL_HEIGHT: u32 = 8;
const CHAR_ROM_SIZE: usize = 0x800;
const FLASH_RATE: u32 = 4;
const BYTES_PER_CELL_ROW: usize = CELL_COLS * (CELL_WIDTH * PIXEL_SIZE) as usize;

mod soft_switch {
    pub const GFX_MODE: usize = 0xC050;
    pub const TXT_MODE: usize = 0xC051;
    pub const SINGLE_MODE: usize = 0xC052;
    pub const MIXED_MODE: usize = 0xC053;
    pub const PG1_MODE: usize = 0xC054;
    pub const PG2_MODE: usize = 0xC055;
    pub const LORES_MODE: usize = 0xC056;
    pub const HIRES_MODE: usize = 0xC057;
}

pub struct GraphicsHandler<'a> {
    canvas: &'a mut Canvas<Window>,
    pixel_buf: [u8; (WIN_WIDTH * WIN_HEIGHT * PIXEL_SIZE) as usize],
    pixel_surface: Texture<'a>,
    char_data: [u8; CHAR_ROM_SIZE],
    frame_count: u32,
    flash: bool,
    txt_mode: bool,
    hires_mode: bool,
    mixed_mode: bool,
    use_pg2: bool
}

fn load_char_set() -> [u8; CHAR_ROM_SIZE] {
    let mut char_rom = File::open(
        "roms/firmware/char_set.rom"
    ).expect("Failed to open charset ROM!");

    let mut char_array = [0; CHAR_ROM_SIZE];
    char_rom.read_exact(&mut char_array).expect("Failed to read char ROM data!");
    char_array
}

fn cell_to_pbuf_idx(cell_idx: usize) -> usize {
    let row = (cell_idx / CELL_COLS) as u32;
    let col = (cell_idx % CELL_COLS) as u32;

(row * (CELL_HEIGHT * BYTES_PER_CELL_ROW as u32) + col * (CELL_WIDTH * PIXEL_SIZE)) as usize
}

impl <'a> GraphicsHandler<'a> {
    fn handle_flash(&mut self, frame_rate: u32) {
        self.frame_count += 1;
        if self.frame_count >= frame_rate / FLASH_RATE {
            self.flash = !self.flash;
            self.frame_count = 0;
        }
    }

    fn draw_pixel(&mut self, color: u32, idx: usize) {
        self.pixel_buf[idx] =     ((color >> 16) & 0xFF) as u8;
        self.pixel_buf[idx + 1] = ((color >>  8) & 0xFF) as u8;
        self.pixel_buf[idx + 2] = ((color >>  0) & 0xFF) as u8;
    }

    fn draw_char(&mut self, val: u8, cell_idx: usize) {
        // Mask off the upper two bits as they don't affect address
        // Then multiply by 8 (since each character is represented by 8 bytes)
        let char_addr = ((val & 0x3F) as u32 * CELL_HEIGHT) as usize;

        // For every byte (row) in character map
        let mut pbuf_idx = cell_to_pbuf_idx(cell_idx);
        for i in char_addr..char_addr + CELL_HEIGHT as usize {
            let mut idx = pbuf_idx;
            let mut char_map = self.char_data[i];

            // 7th bit tells us if in invert mode
            // 6th bit tells us if in flash mode
            // So invert bits if in invert mode, or in flash mode and invert_text is true
            if (val & (1 << 7) == 0) && (val & (1 << 6) == 0 || self.flash) {
                char_map ^= 0xFF; // Invert all bits
            }
            char_map <<= 1; // Then shift off high bit because we don't need it

            // For every dot in row of character map
            for _ in 0..CELL_WIDTH {
                let color = match char_map & (1 << 7) != 0 {
                    true => PIXEL_ON_COLOR,
                    false => PIXEL_OFF_COLOR
                };

                self.draw_pixel(color, idx);
                char_map <<= 1;
                idx += PIXEL_SIZE as usize;
            }

            pbuf_idx += BYTES_PER_CELL_ROW;
        }
    }

    fn draw_lores(&mut self, val: u8, cell_idx: usize) {
        let color_map = [
            0x000000, 0x901740, 0x402CA5, 0xD043E5,
            0x006940, 0x808080, 0x2F95E5, 0xBFABFF,
            0x405400, 0xD06A1A, 0x808080, 0xFF96BF,
            0x2FBC1A, 0xBFD35A, 0x6FE8BF, 0xFFFFFF
        ];

        // Each nybble represents the top half and bottom half colors of a cell block
        // A lookup table is used to map the nybble value to a color
        let lower_color = color_map[(val >> 4) as usize] as u32;
        let upper_color = color_map[(val & 0xF) as usize] as u32;

        let mut pbuf_idx = cell_to_pbuf_idx(cell_idx);
        for j in 0..CELL_HEIGHT {
            let mut idx = pbuf_idx;

            for _ in 0..CELL_WIDTH {
                let color = match j < (CELL_HEIGHT / 2) {
                    true => upper_color,
                    false => lower_color
                };

                self.draw_pixel(color, idx);
                idx += PIXEL_SIZE as usize;
            }

            pbuf_idx += BYTES_PER_CELL_ROW;
        }
    }

    fn handle_lores_gfx(&mut self, buffer: &[u8]) {
        let start_addrs = match self.use_pg2 {
            true  => [0x800, 0x828, 0x850],
            false => [0x400, 0x428, 0x450]
        };

        let mut cell_idx = 0;
        for (section, start) in start_addrs.iter().enumerate() {
            for j in 0..(CELL_ROWS / 3) {
                for i in 0..CELL_COLS {
                    let idx = start + 0x80 * j + i;
                    let row = (section * 8) + j;

                    match self.txt_mode || (row >= 20 && self.mixed_mode) {
                        true => self.draw_char(buffer[idx], cell_idx),
                        false => self.draw_lores(buffer[idx], cell_idx)
                    }

                    cell_idx += 1;
                }
            }
        }
    }

    pub fn handle_gfx(&mut self, frame_rate: u32, buffer: &[u8]) {
        if self.hires_mode {
            // TODO
        } else {
            self.handle_lores_gfx(buffer);
        }

        // Update canvas
        self.pixel_surface.update(
            None,
            &self.pixel_buf, 
            (WIN_WIDTH * PIXEL_SIZE) as usize).unwrap();
        self.canvas.copy(&self.pixel_surface, None, None).unwrap();
        self.canvas.present();

        // Keep track when to "flash" text
        self.handle_flash(frame_rate);
    }

    pub fn handle_soft_sw(&mut self, address: usize) {
        match address {
            soft_switch::GFX_MODE => {
                self.txt_mode = false;
            },
            soft_switch::TXT_MODE => {
                self.txt_mode = true;
            },
            soft_switch::SINGLE_MODE => {
                self.mixed_mode = false;
            },
            soft_switch::MIXED_MODE => {
                self.mixed_mode = true;
            },
            soft_switch::PG1_MODE => {
                self.use_pg2 = false;
            },
            soft_switch::PG2_MODE => {
                self.use_pg2 = true;
            },
            soft_switch::LORES_MODE => {
                self.hires_mode = false;
            },
            soft_switch::HIRES_MODE => {
                self.hires_mode = true
            },
            _ => {}
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
            char_data: load_char_set(),
            frame_count: 0,
            flash: false,
            txt_mode: true,
            hires_mode: false,
            mixed_mode: false,
            use_pg2: false
        }
    }
}