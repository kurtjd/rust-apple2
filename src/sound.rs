use sdl2::audio::{AudioDevice, AudioCallback, AudioSpecDesired};

const SAMPLE_BUF_SZ: usize = 1024;

pub struct SquareWave {
    buffer: [f32; SAMPLE_BUF_SZ],
    sample_idx: usize,
    buf_idx: usize
}

impl SquareWave {
    pub fn insert_sample(&mut self, sample: f32) {
        self.buffer[self.buf_idx] = sample;

        self.buf_idx += 1;
        if self.buf_idx >= SAMPLE_BUF_SZ {
            self.buf_idx = 0;
        }
    }
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        for x in out.iter_mut() {
            if self.sample_idx == self.buf_idx {
                *x = 0.0;
                return;
            }

            *x = self.buffer[self.sample_idx];

            self.sample_idx += 1;
            if self.sample_idx >= SAMPLE_BUF_SZ {
                self.sample_idx = 0;
            }
        }
    }
}

pub struct SoundHandler {
    pub device: AudioDevice<SquareWave>
}

impl SoundHandler {
    pub fn new(sdl_context: &sdl2::Sdl) -> Self {
        let audio_subsystem = sdl_context.audio().unwrap();

        let audio_spec = AudioSpecDesired {
            freq: Some(44100),
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
            device
        }
    }
}