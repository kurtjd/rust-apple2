const MEM_SIZE: usize = 0x10000;
const ROM_START: usize = 0xC000;

#[derive(Clone)]
pub struct Cycle {
    pub address: usize,
    pub value: u8,
    pub ctype: String
}

pub struct MemManager {
    pub memory: [u8; MEM_SIZE],
    pub cycles: Vec<Cycle>
}

impl MemManager {
    pub fn new() -> Self {
        MemManager {
            memory: [0; MEM_SIZE],
            cycles: Vec::new()
        }
    }

    // These are used by the CPU
    pub fn mem_read(&mut self, address: usize) -> u8 {
        let value = self.memory[address];

        self.cycles.push(Cycle {
            address,
            value,
            ctype: "read".to_string()
        });
        
        value
    }

    pub fn mem_write(&mut self, address: usize, value: u8) {
        if address < ROM_START {
            self.memory[address] = value;
        }

        // So we can check write activity
        self.cycles.push(Cycle {
            address,
            value,
            ctype: "write".to_string()
        });
    }

    // Used by the Apple 2 emulator
    pub fn get_cycles(&self) -> Vec<Cycle> {
        // Yeah we do a copy otherwise borrow checker yells...
        self.cycles.clone()
    }

    pub fn clear_cycles(&mut self) {
        self.cycles.clear();
    }

    pub fn handle_soft_sw(&mut self) {

    }
}