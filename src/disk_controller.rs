use std::path::Path;
use crate::wizard_of_woz::WozImage;

const WOZ_IMG_SIZE: usize = 234496;
const TRK_BUF_SIZE: usize = 220080;

const MAX_TRACK: u8 = 34;
const TRACK0_ADDR: usize = 0x600;
const BLOCK_SIZE: usize = 512;
const BLOCKS_PER_TRACK: usize = 13;
const TRACK_BYTES_RESERVED: usize = BLOCKS_PER_TRACK * BLOCK_SIZE;
const BITS_PER_TRACK: usize = 50304;
const BYTES_PER_TRACK: usize = BITS_PER_TRACK / 8;

const PERIPH_IO_ADDR: usize = 0xC080;

mod soft_switch {
    use super::PERIPH_IO_ADDR;

    pub const PHASE0_OFF: usize = PERIPH_IO_ADDR + 0x0;
    pub const PHASE1_OFF: usize = PERIPH_IO_ADDR + 0x2;
    pub const PHASE2_OFF: usize = PERIPH_IO_ADDR + 0x4;
    pub const PHASE3_OFF: usize = PERIPH_IO_ADDR + 0x6;
    pub const DRIVES_OFF: usize = PERIPH_IO_ADDR + 0x8;
    pub const SEL_DRIVE1: usize = PERIPH_IO_ADDR + 0xA;
    pub const SHIFT_OFF: usize  = PERIPH_IO_ADDR + 0xC;
    pub const DISK_READ: usize  = PERIPH_IO_ADDR + 0xE;
    pub const PHASE0_ON: usize  = PERIPH_IO_ADDR + 0x1;
    pub const PHASE1_ON: usize  = PERIPH_IO_ADDR + 0x3;
    pub const PHASE2_ON: usize  = PERIPH_IO_ADDR + 0x5;
    pub const PHASE3_ON: usize  = PERIPH_IO_ADDR + 0x7;
    pub const DRIVES_ON: usize  = PERIPH_IO_ADDR + 0x9;
    pub const SEL_DRIVE2: usize = PERIPH_IO_ADDR + 0xB;
    pub const SHIFT_ON: usize   = PERIPH_IO_ADDR + 0xD;
    pub const DISK_WRITE: usize = PERIPH_IO_ADDR + 0xF;
}

pub struct DiskController {
    slot: usize,
    data_reg: u8,
    half_track: u8,
    next_phase: u8,
    bit_pntr: usize,
    drives_on: bool,
    current_drive: u8,
    write_mode: bool,
    write_sense: bool,
    disk_image: Option<WozImage>,
    motor_off_delay: u8
}

impl DiskController {
    pub fn new(slot: usize) -> Self {
        DiskController {
            slot,
            data_reg: 0,
            half_track: 0,
            next_phase: 0,
            bit_pntr: 0,
            drives_on: false,
            current_drive: 1,
            write_mode: false,
            write_sense: false,
            disk_image: None,
            motor_off_delay: 0
        }
    }

    fn phase_on(&mut self, phase: u8) {
        if self.drives_on {
            self.next_phase = phase;
        }
    }

    fn phase_off(&mut self, phase: u8) {
        if !self.drives_on {
            return;
        }

        // If phases turned off in descending order, track increases
        if (self.next_phase > phase || (self.next_phase == 0 && phase == 3)) &&
            self.half_track < MAX_TRACK * 2
        {
            self.half_track += 1;
        }

        // Likewise if phases turned off in ascending order, track decreases
        else if (self.next_phase < phase || (self.next_phase == 3 && phase == 0)) &&
                self.half_track > 0
        {
            self.half_track -= 1;
        }
    }

    pub fn load_image(&mut self, image_path: &Path) {
        self.disk_image = Some(
            WozImage::new(image_path).unwrap()
        );
    }

    fn get_next_bit(&mut self) -> u8 {
        let track_idx = (self.half_track / 2) as usize;
        let track = &(self.disk_image.as_ref().unwrap().tracks[track_idx]);
        let track_data = &track.data;

        let byte_idx = self.bit_pntr / 8;
        let bit_on = self.bit_pntr % 8;
        let byte = track_data[byte_idx];
        let bit = (byte >> (7 - bit_on)) & 1;

        // Wrap around to simulate disk spinning in circle
        self.bit_pntr += 1;
        if self.bit_pntr >= track.bit_count as usize {
            self.bit_pntr = 0;
        }

        bit
    }

    fn get_next_byte(&mut self) {
        let mut bit = self.get_next_bit();
        
        /* If we receive a 0, we are in the middle of a 10-bit self-sync byte so keep reading
        until at the beginning of a valid disk byte */
        while bit == 0 {
            bit = self.get_next_bit();
        }

        // Once found the beginning of valid byte, shift in the next 7 bits
        self.data_reg = 1;
        for _ in 0..7 {
            self.data_reg <<= 1;
            self.data_reg |= self.get_next_bit();
        }
    }

    fn load_byte(&mut self, address: usize, ram: &mut[u8]) {
        if self.drives_on && !self.write_mode {
            // If in write-protect sense mode, return whether or not disk is write protected
            if self.write_sense {
                self.data_reg = 1 << 7; // Lets just say always write protected for now
            } else {
                self.get_next_byte();
            }
        }

        ram[address] = self.data_reg;
    }

    pub fn handle_motor_off_delay(&mut self) {
        /* When drives are turned off, there is actually a one second delay before they actually
        turn off. This is called every frame cycle starting at a count of 60. */
        if self.motor_off_delay > 0 {
            self.motor_off_delay -= 1;

            if self.motor_off_delay == 0 {
                self.drives_on = false;
            }
        }
    }

    pub fn handle_soft_sw(&mut self, address: usize, ram: &mut[u8]) {
        if address < PERIPH_IO_ADDR {
            return;
        }

        // TODO: Reset should force all switches off

        match address - self.slot {
            // Off
            soft_switch::PHASE0_OFF => {
                self.phase_off(0);
                self.load_byte(address, ram);
            },
            soft_switch::PHASE1_OFF => {
                self.phase_off(1);
                self.load_byte(address, ram);
            },
            soft_switch::PHASE2_OFF => {
                self.phase_off(2);
                self.load_byte(address, ram);
            },
            soft_switch::PHASE3_OFF => {
                self.phase_off(3);
                self.load_byte(address, ram);
            },
            soft_switch::DRIVES_OFF => {
                self.motor_off_delay = 60; // 60 frames per second
                self.load_byte(address, ram);
            },
            soft_switch::SEL_DRIVE1 => {
                self.current_drive = 1;
                self.load_byte(address, ram);
            },
            soft_switch::SHIFT_OFF => {
                self.write_sense = false;
                if !self.write_mode {
                    self.load_byte(address, ram);
                } else {
                    // TODO: Actually write data to disk image
                    // Copy data reg to disk byte pointer
                    // I assume CPU waits for sequencer to shift out bits?
                    // So can just do it in one go?
                }
            },
            soft_switch::DISK_READ => {
                self.write_mode = false;
                self.load_byte(address, ram);
            },

            // On
            soft_switch::PHASE0_ON => {
                self.phase_on(0);
            },
            soft_switch::PHASE1_ON => {
                self.phase_on(1);
            },
            soft_switch::PHASE2_ON => {
                self.phase_on(2);
            },
            soft_switch::PHASE3_ON => {
                self.phase_on(3);
            },
            soft_switch::DRIVES_ON => {
                self.drives_on = true;
                self.motor_off_delay = 0;
            },
            soft_switch::SEL_DRIVE2 => {
                self.current_drive = 2;
            },
            soft_switch::SHIFT_ON => {
                self.write_sense = true;
                if self.write_mode {
                    self.data_reg = ram[address];
                } else {
                    self.data_reg = 0; // Apprently reading this addr clears data register
                }
            },
            soft_switch::DISK_WRITE => {
                self.write_mode = true;
            },
            _ => {}
        }
    }
}