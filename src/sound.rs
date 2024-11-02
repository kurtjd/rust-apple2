use sdl2::audio::{AudioDevice, AudioCallback, AudioSpecDesired};

const SAMPLE_BUF_SZ: usize = 1024;
const SAMPLE_VOLUME: f32 = 0.5;
pub const SAMPLE_RATE: u32 = 44100;

mod soft_switch {
    pub const SPEAKER: usize = 0xC030; // Whole page
}

pub struct SquareWave {
    buffer: [f32; SAMPLE_BUF_SZ],
    sample_idx: usize,
    buf_idx: usize
}

impl SquareWave {
    pub fn insert_sample(&mut self, sample: f32) {
        self.buffer[self.buf_idx] = sample;
        self.buf_idx += 1;
        self.buf_idx %= SAMPLE_BUF_SZ;
    }
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        for x in out.iter_mut() {
            if self.sample_idx == self.buf_idx {
                *x = 0.0;
            } else {
                *x = self.buffer[self.sample_idx];
                self.sample_idx += 1;
                self.sample_idx %= SAMPLE_BUF_SZ;
            }
        }
    }
}

pub struct SoundHandler {
    pub device: AudioDevice<SquareWave>,
    pub polarity: bool
}

impl SoundHandler {
    pub fn new(sdl_context: &sdl2::Sdl) -> Self {
        let audio_subsystem = sdl_context.audio().unwrap();

        let audio_spec = AudioSpecDesired {
            freq: Some(SAMPLE_RATE as i32),
            channels: Some(1),
            samples: Some(512)
        };
    
        let wave = SquareWave {
            buffer: [0.0; SAMPLE_BUF_SZ],
            sample_idx: 0,
            buf_idx: 0
        };

        let device = audio_subsystem.open_playback(None, &audio_spec, |_| { wave }).unwrap();

        SoundHandler {
            device,
            polarity: false
        }
    }
    
    pub fn insert_samples(&mut self, samples: &Vec<bool>) {
        let mut lock = self.device.lock();
        for s in samples {
            lock.insert_sample(match s {
                true  =>  SAMPLE_VOLUME,
                false => -SAMPLE_VOLUME
            });
        }
    }

    pub fn handle_soft_sw(&mut self, address: usize) {
        /*match address {
            soft_switch::SPEAKER => {
                self.polarity = !self.polarity;
            },
            _ => {}
        }*/
        if address == soft_switch::SPEAKER {
            self.polarity = !self.polarity;
        }
    }
}
