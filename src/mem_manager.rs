const MEM_SIZE: usize = 0x10000;
const ROM_START: usize = 0xC000;
const BANK_RAM_START: usize = 0xD000;
const BANK_RAM_SIZE: usize = 0x1000;
const EXT_RAM_START: usize = 0xE000;
const EXT_RAM_SIZE: usize = 0x2000;

const WRITE_EN_COUNT_MAX: u8 = 1;

mod soft_switch {
    pub const BANK2_RAM_READ_NO_WRITE: usize = 0xC080;
    pub const BANK2_ROM_READ_WRITE: usize = 0xC081;
    pub const BANK2_ROM_READ_NO_WRITE: usize = 0xC082;
    pub const BANK2_RAM_READ_WRITE: usize = 0xC083;
    pub const BANK1_RAM_READ_NO_WRITE: usize = 0xC088;
    pub const BANK1_ROM_READ_WRITE: usize = 0xC089;
    pub const BANK1_ROM_READ_NO_WRITE: usize = 0xC08A;
    pub const BANK1_RAM_READ_WRITE: usize = 0xC08B;

    pub const BANK2_RAM_READ_NO_WRITE_ALT: usize = BANK2_RAM_READ_NO_WRITE + 4;
    pub const BANK2_ROM_READ_WRITE_ALT: usize = BANK2_ROM_READ_WRITE + 4;
    pub const BANK2_ROM_READ_NO_WRITE_ALT: usize = BANK2_ROM_READ_NO_WRITE + 4;
    pub const BANK2_RAM_READ_WRITE_ALT: usize = BANK2_RAM_READ_WRITE + 4;
    pub const BANK1_RAM_READ_NO_WRITE_ALT: usize = BANK1_RAM_READ_NO_WRITE + 4;
    pub const BANK1_ROM_READ_WRITE_ALT: usize = BANK1_ROM_READ_WRITE + 4;
    pub const BANK1_ROM_READ_NO_WRITE_ALT: usize = BANK1_ROM_READ_NO_WRITE + 4;
    pub const BANK1_RAM_READ_WRITE_ALT: usize = BANK1_RAM_READ_WRITE + 4;
}

#[derive(Clone)]
pub struct Cycle {
    pub address: usize,
    pub value: u8,
    pub ctype: String
}

pub struct MemManager {
    pub memory: [u8; MEM_SIZE],
    bank1_ram: [u8; BANK_RAM_SIZE],
    bank2_ram: [u8; BANK_RAM_SIZE],
    ext_ram: [u8; EXT_RAM_SIZE],
    bank2_active: bool,
    rom_read: bool,
    ram_write: bool,
    write_en_count: u8,

    pub cycles: Vec<Cycle>
}

impl MemManager {
    pub fn new() -> Self {
        MemManager {
            memory: [0; MEM_SIZE],
            bank1_ram: [0; BANK_RAM_SIZE],
            bank2_ram: [0; BANK_RAM_SIZE],
            ext_ram: [0; EXT_RAM_SIZE],
            bank2_active: true,
            rom_read: true,
            ram_write: true,
            write_en_count: WRITE_EN_COUNT_MAX,
            cycles: Vec::new(),
        }
    }

    // These are used by the CPU
    pub fn mem_read(&mut self, address: usize) -> u8 {
        let value = match address < BANK_RAM_START || self.rom_read {
            true => self.memory[address],

            false => match address < EXT_RAM_START {
                true => match self.bank2_active {
                    true => self.bank2_ram[address - BANK_RAM_START],
                    false => self.bank1_ram[address - BANK_RAM_START]
                },

                false => self.ext_ram[address - EXT_RAM_START]
            }
        };

        self.cycles.push(Cycle {
            address,
            value,
            ctype: "read".to_string()
        });
        
        value
    }

    pub fn mem_write(&mut self, address: usize, value: u8) {
        self.cycles.push(Cycle {
            address,
            value,
            ctype: "write".to_string()
        });

        match address < ROM_START {
            true => self.memory[address] = value,

            false => match self.ram_write {
                true => match address >= BANK_RAM_START {
                    true => match address < EXT_RAM_START {
                        true => match self.bank2_active {
                            true => self.bank2_ram[address - BANK_RAM_START] = value,
                            false => self.bank1_ram[address - BANK_RAM_START] = value
                        },

                        false => self.ext_ram[address - EXT_RAM_START] = value
                    }, 

                    false => {}
                },

                false => {}
            }
        }
    }

    // Used by the Apple 2 emulator
    pub fn reset(&mut self) {
        self.bank2_active = true;
        self.rom_read = true;
        self.ram_write = true;
        self.write_en_count = WRITE_EN_COUNT_MAX;
    }

    pub fn get_cycles(&self) -> Vec<Cycle> {
        // Yeah we do a copy otherwise borrow checker yells...
        self.cycles.clone()
    }

    pub fn clear_cycles(&mut self) {
        self.cycles.clear();
    }

    pub fn handle_soft_sw(&mut self, address: usize, ctype: &String) {
        /* Only respond to read requests. But if we receive a write, reset the write enable count
        because technically it requires two READs to become enabled. */
        if ctype == "write" {
            self.write_en_count = WRITE_EN_COUNT_MAX;
            return;
        }

        match address {
            soft_switch::BANK2_RAM_READ_NO_WRITE | soft_switch::BANK2_RAM_READ_NO_WRITE_ALT => {
                self.read_enable(true, false);
            },
            soft_switch::BANK2_ROM_READ_WRITE | soft_switch::BANK2_ROM_READ_WRITE_ALT => {
                self.write_enable(true, true);
            },
            soft_switch::BANK2_ROM_READ_NO_WRITE | soft_switch::BANK2_ROM_READ_NO_WRITE_ALT => {
                self.read_enable(true, true);
            },
            soft_switch::BANK2_RAM_READ_WRITE | soft_switch::BANK2_RAM_READ_WRITE_ALT => {
                self.write_enable(true, false);
            },
            soft_switch::BANK1_RAM_READ_NO_WRITE | soft_switch::BANK1_RAM_READ_NO_WRITE_ALT => {
                self.read_enable(false, false);
            },
            soft_switch::BANK1_ROM_READ_WRITE | soft_switch::BANK1_ROM_READ_WRITE_ALT => {
                self.write_enable(false, true);
            },
            soft_switch::BANK1_ROM_READ_NO_WRITE | soft_switch::BANK1_ROM_READ_NO_WRITE_ALT => {
                self.read_enable(false, true);
            },
            soft_switch::BANK1_RAM_READ_WRITE | soft_switch::BANK1_RAM_READ_WRITE_ALT => {
                self.write_enable(false, false);
            },
            _ => {}
        }
    }

    fn read_enable(&mut self, bank2: bool, rom_read: bool) {
        self.bank2_active = bank2;
        self.rom_read = rom_read;
        self.ram_write = false;
        self.write_en_count = WRITE_EN_COUNT_MAX;
    }

    fn write_enable(&mut self, bank2: bool, rom_read: bool) {
        self.bank2_active = bank2;
        self.rom_read = rom_read;

        // It takes two consecutive accesses to a write enable switch to actually enable RAM write
        if !self.ram_write {
            if self.write_en_count == 0 {
                self.ram_write = true;
                self.write_en_count = WRITE_EN_COUNT_MAX;
            } else {
                self.write_en_count -= 1;
            }
        }
    }
}