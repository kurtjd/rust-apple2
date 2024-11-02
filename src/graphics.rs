use sdl2::pixels::PixelFormatEnum;
use sdl2::render::TextureCreator;
use sdl2::video::WindowContext;
use sdl2::{video::Window, render::Canvas, render::Texture};

use std::{fs::File, io::Read};

pub const DISP_WIDTH: u32 = 280;
pub const DISP_HEIGHT: u32 = 192;
pub const DISP_SCALE: u32 = 3;

const PIXEL_SIZE: u32 = 3;
const BLOCK_ROWS: usize = 24;
const BLOCK_COLS: usize = 40;
const BLOCK_WIDTH: u32 = 7;
const BLOCK_HEIGHT: u32 = 8;
const CHAR_ROM_SIZE: usize = 0x800;
const FLASH_RATE: u32 = 4;
const BYTES_PER_BLOCK_ROW: usize = BLOCK_COLS * (BLOCK_WIDTH * PIXEL_SIZE) as usize;

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

mod color {
    // All
    pub const BLACK: u32 = 0x000000;
    pub const WHITE: u32 = 0xFFFFFF;

    // LORES
    pub const MAGENTA: u32 = 0x901740;
    pub const DARK_BLUE: u32 = 0x402CA5;
    pub const PURPLE: u32 = 0xD043E5;
    pub const DARK_GREEN: u32 = 0x006940;
    pub const GREY1: u32 = 0x808080;
    pub const BLUE: u32 = 0x2F95E5;
    pub const LIGHT_BLUE: u32 = 0xBFABFF;
    pub const BROWN: u32 = 0x405400;
    pub const ORANGE: u32 = 0xD06A1A;
    pub const GREY2: u32 = 0x808080;
    pub const PINK: u32 = 0xFF96BF;
    pub const LIGHT_GREEN: u32 = 0x2FBC1A;
    pub const YELLOW: u32 = 0xBFD35A;
    pub const AQUA: u32 = 0x6FE8BF;

    // HIRES
    pub const HIRES_BLUE: u32 = 0x4BB8F1;
    pub const HIRES_ORANGE: u32 = 0xE6792E;
    pub const HIRES_VIOLET: u32 = 0xD660EF;
    pub const HIRES_GREEN: u32 = 0x68E043;
}

pub struct GraphicsHandler<'a> {
    canvas: &'a mut Canvas<Window>,
    pixel_buf: [u8; (DISP_WIDTH * DISP_HEIGHT * PIXEL_SIZE) as usize],
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

fn block_to_pbuf_idx(block_idx: usize) -> usize {
    let row = (block_idx / BLOCK_COLS) as u32;
    let col = (block_idx % BLOCK_COLS) as u32;

    (row * (BLOCK_HEIGHT * BYTES_PER_BLOCK_ROW as u32) + col * (BLOCK_WIDTH * PIXEL_SIZE)) as usize
}

fn to_pixel_map(buffer: &[u8], buf_idx: usize, block_col: usize) -> [u32; BLOCK_WIDTH as usize] {
    // The pixels for this block in order
    let mut pixel_map = [color::BLACK; BLOCK_WIDTH as usize];

    let val = buffer[buf_idx];
    let alt_colors = (val >> 7) != 0; // If MSB is high, use alternate color palette
    
    // We need to check bordering dots, even if in adjacent bytes
    let left_block_dot = match block_col != 0 {
        true => (buffer[buf_idx - 1] >> 6) & 1,
        false => 0
    };
    let right_block_dot = match block_col != (BLOCK_COLS - 1) {
        true => buffer[buf_idx + 1] & 1,
        false => 0
    };

    /* Scan each bit (except the MSB), mapping it to a color depending on its value and its
    neighboring bits . */
    let mut tmp = val;
    for (i, pixel) in pixel_map.iter_mut().enumerate() {
        let dot = tmp & 1;
        let left_dot = match i == 0 {
            true => left_block_dot,
            false => val >> (i - 1) & 1
        };
        let right_dot = match i == 6 {
            true => right_block_dot,
            false => val >> (i + 1) & 1
        };
        
        // "Evenness" depends on block column and position within block
        let is_even = ((block_col % 2 == 0) && (i % 2 == 0)) ||
                      ((block_col % 2 == 1) && (i % 2 == 1));

        let color = if dot != 0 {
            // Any high bit bordering another high bit becomes a white dot
            if left_dot == 1 || right_dot == 1 {
                color::WHITE 
            } else if alt_colors && is_even {
                color::HIRES_BLUE
            } else if alt_colors && !is_even {
                color::HIRES_ORANGE
            } else if !alt_colors && is_even {
                color::HIRES_VIOLET
            } else {
                color::HIRES_GREEN
            }
        
        /* If the bit is low, but borders a high bit to its right, incorporate "fringing"
        This is not perfect, fringing is difficult to get right with its half pixel shifts
        and whatnot. */
        } else if right_dot == 1 {
            if alt_colors && !is_even {
                color::HIRES_BLUE
            } else if alt_colors && is_even {
                color::HIRES_ORANGE
            } else if !alt_colors && !is_even {
                color::HIRES_VIOLET
            } else {
                color::HIRES_GREEN
            }
        
        // Otherwise just draw a black pixel
        } else {
            color::BLACK
        };

        tmp >>= 1;
        *pixel = color;
    }

    pixel_map
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
        for i in 0..PIXEL_SIZE as usize {
            self.pixel_buf[idx + i] = ((color >> (16 - (8 * i))) & 0xFF) as u8;
        }
    }

    fn draw_char_block(&mut self, val: u8, block_idx: usize) {
        // Mask off the upper two bits as they don't affect address
        // Then multiply by 8 (since each character is represented by 8 bytes)
        let char_addr = ((val & 0x3F) as u32 * BLOCK_HEIGHT) as usize;

        // For every byte (row) in character map
        let mut pbuf_idx = block_to_pbuf_idx(block_idx);
        for j in char_addr..char_addr + BLOCK_HEIGHT as usize {
            let mut idx = pbuf_idx;
            let mut char_map = self.char_data[j];

            // 7th bit tells us if in invert mode
            // 6th bit tells us if in flash mode
            // So invert bits if in invert mode, or in flash mode and invert_text is true
            if (val & (1 << 7) == 0) && (val & (1 << 6) == 0 || self.flash) {
                char_map ^= 0xFF; // Invert all bits
            }

            // If in hires mode, convert text row into hires pixel mapping
            let mut pixel_map: Option<[u32; BLOCK_WIDTH as usize]> = None;
            if self.hires_mode {
                let block_col = block_idx % BLOCK_COLS;
                
                // Need to reverse the 6 LSBs because hires mode has things reversed
                let mut reverse = 0;
                for k in 0..7 {
                    if (char_map & (1 << k)) != 0 {
                        reverse |= 1 << (6 - k);
                    }
                }
                
                // Create this "buffer" because that's what pixel map expects for hires mode
                let buffer = [0, reverse, 0];
                pixel_map = Some(to_pixel_map(&buffer, 1, block_col));
            } else {
                char_map <<= 1; // Then shift off high bit because we don't need it
            }

            // For every dot in row of character map
            for i in 0..BLOCK_WIDTH {
                if let Some(pixel_map) = pixel_map {
                    self.draw_pixel(pixel_map[i as usize], idx);
                } else {
                    let color = match char_map & (1 << 7) != 0 {
                        true  => color::WHITE,
                        false => color::BLACK
                    };

                    self.draw_pixel(color, idx);
                    char_map <<= 1;
                }

                idx += PIXEL_SIZE as usize;
            }

            pbuf_idx += BYTES_PER_BLOCK_ROW;
        }
    }

    fn draw_lores_block(&mut self, val: u8, block_idx: usize) {
        let color_map = [
            color::BLACK, color::MAGENTA, color::DARK_BLUE, color::PURPLE,
            color::DARK_GREEN, color::GREY1, color::BLUE, color::LIGHT_BLUE,
            color::BROWN, color::ORANGE, color::GREY2, color::PINK,
            color::LIGHT_GREEN, color::YELLOW, color::AQUA, color::WHITE
        ];

        // Each nybble represents the top half and bottom half colors of a block
        // A lookup table is used to map the nybble value to a color
        let lower_color = color_map[(val >> 4) as usize];
        let upper_color = color_map[(val & 0xF) as usize];

        let mut pbuf_idx = block_to_pbuf_idx(block_idx);
        for j in 0..BLOCK_HEIGHT {
            let mut idx = pbuf_idx;

            for _ in 0..BLOCK_WIDTH {
                let color = match j < (BLOCK_HEIGHT / 2) {
                    true => upper_color,
                    false => lower_color
                };

                self.draw_pixel(color, idx);
                idx += PIXEL_SIZE as usize;
            }

            pbuf_idx += BYTES_PER_BLOCK_ROW;
        }
    }

    fn draw_hires_block(&mut self, buffer: &[u8], buf_idx: usize, block_idx: usize) {
        let block_col = block_idx % BLOCK_COLS;
        let mut pbuf_idx = block_to_pbuf_idx(block_idx);

        for j in 0..BLOCK_HEIGHT {
            let mut idx = pbuf_idx;
            let pixel_map = to_pixel_map(buffer, buf_idx + (j as usize * 1024), block_col);

            for i in 0..BLOCK_WIDTH {
                self.draw_pixel(pixel_map[i as usize], idx);
                idx += PIXEL_SIZE as usize;
            }

            pbuf_idx += BYTES_PER_BLOCK_ROW;
        }
    }

    /* The Apple 2 video memory mapping is crazy (though it makes sense why it is
        the way that it is). So don't blame me for this insanity! */
    fn draw_blocks(&mut self, buffer: &[u8]) {
        let lores_addrs = match self.use_pg2 {
            true  => [0x800, 0x828, 0x850],
            false => [0x400, 0x428, 0x450]
        };
        let hires_addrs = match self.use_pg2 {
            true  => [0x4000, 0x4028, 0x4050],
            false => [0x2000, 0x2028, 0x2050]
        };

        let start_addrs = match self.hires_mode {
            true  => &hires_addrs,
            false => &lores_addrs,
        };

        // Regardless of gfx mode, we draw 7x8 pixel "blocks" one at a time
        let mut block_idx = 0;
        for (section, start) in start_addrs.iter().enumerate() {
            for j in 0..(BLOCK_ROWS / 3) {
                for i in 0..BLOCK_COLS {
                    let offset = 0x80 * j + i;
                    let idx = start + offset;
                    let txt_idx = lores_addrs[section] + offset;
                    let block_row = (section * 8) + j;

                    // If in mixed mode, always draw characters in the last 4 block rows
                    match self.txt_mode || (block_row >= 20 && self.mixed_mode) {
                        true  => self.draw_char_block(buffer[txt_idx], block_idx),
                        false => match self.hires_mode {
                            true  => self.draw_hires_block(buffer, idx, block_idx),
                            false => self.draw_lores_block(buffer[idx], block_idx)
                        }
                    }

                    block_idx += 1;
                }
            }
        }
    }

    pub fn handle_gfx(&mut self, frame_rate: u32, buffer: &[u8]) {
        self.draw_blocks(buffer);

        // Update canvas
        self.pixel_surface.update(
            None,
            &self.pixel_buf, 
            (DISP_WIDTH * PIXEL_SIZE) as usize).unwrap();
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
                self.hires_mode = false;
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
            pixel_buf: [0; (DISP_WIDTH * DISP_HEIGHT * PIXEL_SIZE) as usize],
            pixel_surface: texture_creator.create_texture_static(
                PixelFormatEnum::RGB24,
                DISP_WIDTH,
                DISP_HEIGHT).unwrap(),
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
