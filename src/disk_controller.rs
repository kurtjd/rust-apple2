#![allow(dead_code)]

const MAX_TRACKS: u8 = 35;

mod soft_switch {
    pub const PHASE0_OFF: usize = 0xC080;
    pub const PHASE1_OFF: usize = 0xC082;
    pub const PHASE2_OFF: usize = 0xC084;
    pub const PHASE3_OFF: usize = 0xC086;
    pub const DRIVES_OFF: usize = 0xC088;
    pub const SEL_DRIVE1: usize = 0xC08A;
    pub const SHIFT_READ: usize = 0xC08C;
    pub const DISK_READ: usize = 0xC08E;
    pub const PHASE0_ON: usize = 0xC081;
    pub const PHASE1_ON: usize = 0xC083;
    pub const PHASE2_ON: usize = 0xC085;
    pub const PHASE3_ON: usize = 0xC087;
    pub const DRIVES_ON: usize = 0xC089;
    pub const SEL_DRIVE2: usize = 0xC08B;
    pub const SHIFT_WRITE: usize = 0xC08D;
    pub const DISK_WRITE: usize = 0xC08F;
}

pub struct DiskController {
    slot: usize,
    half_track: u8,
    prev_phase: u8,
    drives_on: bool,
    current_drive: u8
}

impl DiskController {
    pub fn new(slot: usize) -> Self {
        DiskController {
            slot,
            half_track: 0,
            prev_phase: 0,
            drives_on: false,
            current_drive: 1
        }
    }

    fn phase_off(&mut self, phase: u8) {
        self.prev_phase = phase;
    }

    fn phase_on(&mut self, phase: u8) {
        if (phase > self.prev_phase || phase == 0 && self.prev_phase == 3) &&
            self.half_track < MAX_TRACKS * 2
        {
            self.half_track += 1;
        }
        else if (phase < self.prev_phase || phase == 3 && self.prev_phase == 0) &&
                self.half_track > 0
        {
            self.half_track -= 1;
        }
    }

    pub fn handle_soft_sw(&mut self, address: usize) {
        if address < 0xC080 { return; }

        match address - self.slot {
            soft_switch::PHASE0_OFF => {
                self.phase_off(0);
            },
            soft_switch::PHASE1_OFF => {
                self.phase_off(1);
            },
            soft_switch::PHASE2_OFF => {
                self.phase_off(2);
            },
            soft_switch::PHASE3_OFF => {
                self.phase_off(3);
            },
            soft_switch::DRIVES_OFF => {
                self.drives_on = false;
                /* Will likely need to handle the fact that it takes about 1 sec for disk motor
                to actually stop spinning */
            },
            soft_switch::SEL_DRIVE1 => {
                self.current_drive = 1;
            },
            soft_switch::SHIFT_READ => {

            },
            soft_switch::DISK_READ => {

            },
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
            soft_switch::SHIFT_WRITE => {

            },
            soft_switch::DISK_WRITE => {

            },
            _ => {}
        }
    }
}