#![allow(dead_code)]

use std::path::Path;
use std::{fs::File, io::Read};

const MAX_TRACK: u8 = 34;
const WOZ_IMG_SIZE: usize = 234496;
const DSK_IMG_SIZE: usize = 143360;
const GCR_IMG_SIZE: usize = 191520;
const TRK_BUF_SIZE: usize = 220080;

mod soft_switch {
    pub const PHASE0_OFF: usize = 0xC080;
    pub const PHASE1_OFF: usize = 0xC082;
    pub const PHASE2_OFF: usize = 0xC084;
    pub const PHASE3_OFF: usize = 0xC086;
    pub const DRIVES_OFF: usize = 0xC088;
    pub const SEL_DRIVE1: usize = 0xC08A;
    pub const SHIFT_OFF: usize = 0xC08C;
    pub const DISK_READ: usize = 0xC08E;
    pub const PHASE0_ON: usize = 0xC081;
    pub const PHASE1_ON: usize = 0xC083;
    pub const PHASE2_ON: usize = 0xC085;
    pub const PHASE3_ON: usize = 0xC087;
    pub const DRIVES_ON: usize = 0xC089;
    pub const SEL_DRIVE2: usize = 0xC08B;
    pub const SHIFT_ON: usize = 0xC08D;
    pub const DISK_WRITE: usize = 0xC08F;
}

pub struct DiskController {
    slot: usize,
    pub data_reg: u8,
    half_track: u8,
    next_phase: u8,
    bit_pntr: usize,
    drives_on: bool,
    current_drive: u8,
    write_mode: bool,
    write_sense: bool,
    disk1_image: [u8; TRK_BUF_SIZE],
    disk2_image: [u8; TRK_BUF_SIZE],
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
            disk1_image: [0; TRK_BUF_SIZE],
            disk2_image: [0; TRK_BUF_SIZE],
            motor_off_delay: 0
        }
    }

    fn phase_on(&mut self, phase: u8) {
        self.next_phase = phase;
    }

    fn phase_off(&mut self, phase: u8) {
        if (self.next_phase > phase || (self.next_phase == 0 && phase == 3)) &&
            self.half_track < MAX_TRACK * 2
        {
            self.half_track += 1;
        }
        else if (self.next_phase < phase || (self.next_phase == 3 && phase == 0)) &&
                self.half_track > 0
        {
            self.half_track -= 1;
        }
    }

    pub fn load_image(&mut self, image_path: &Path) {
        let mut file_buf = [0; WOZ_IMG_SIZE];
        let mut image = File::open(image_path).expect("Failed to open disk image!");
        image.read(&mut file_buf).expect("Failed to read disk image data!");

        let mut bpntr = 0;
        for i in 0..35 {
            let start = 0x600 + i * 512 * 13;
            for j in 0..6288 {
                self.disk1_image[bpntr] = file_buf[start + j];
                bpntr += 1;
            }
        }
    }

    fn get_next_bit(&mut self) -> u8 {
        let track_idx = (self.half_track / 2) as usize * 6288;
        let byte_idx = track_idx + (self.bit_pntr / 8);
        let bit_on = self.bit_pntr % 8;
        let byte = self.disk1_image[byte_idx];
        let bit = (byte >> (7 - bit_on)) & 1;
        
        self.bit_pntr += 1;
        if self.bit_pntr >= 50304 { // Bits per track
            self.bit_pntr = 0;
        }

        bit
    }

    fn get_next_byte(&mut self) {
        let mut bit = self.get_next_bit();
        while bit == 0 {
            bit = self.get_next_bit();
        }

        self.data_reg = 1;
        for _ in 0..7 {
            self.data_reg <<= 1;
            self.data_reg |= self.get_next_bit();
        }
    }

    fn load_byte(&mut self, address: usize, ram: &mut[u8]) {
        if self.write_sense {
            self.data_reg = 1 << 7; // Lets just say always write protected for now
        } else {
            self.get_next_byte();
        }

        ram[address] = self.data_reg;
    }

    pub fn handle_soft_sw(&mut self, address: usize, ram: &mut[u8]) {
        if address < 0xC080 { return; }

        // Reset should force all switches off

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
                //self.drives_on = false;
                //self.motor_off_delay = 60;
                //self.load_byte(address, ram);
                /* Will likely need to handle the fact that it takes about 1 sec for disk motor
                to actually stop spinning */
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
            },
            soft_switch::SEL_DRIVE2 => {
                self.current_drive = 2;
            },
            soft_switch::SHIFT_ON => {
                self.write_sense = true;
                if self.write_mode {
                    self.data_reg = ram[address];
                } else {
                    self.data_reg = 0; // Reset sequencer?
                }
            },
            soft_switch::DISK_WRITE => {
                self.write_mode = true;
            },
            _ => {}
        }
    }
}