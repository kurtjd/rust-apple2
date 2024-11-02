/*
Wizard of Woz simply parses a raw WOZ2 image and returns a struct containing pertinent info.
Reference: https://applesaucefdc.com/woz/reference2
*/

use std::path::Path;
use std::{fs::File, io::Read};

use crate::dsk2woz;

const WOZ_IMG_SIZE: usize = 250000;

const MAX_TRACKS: usize = 35;

mod section_id {
    pub const WOZ2: u32 = 0x325A4F57;
    pub const INFO: u32 = 0x4F464E49;
    pub const TMAP: u32 = 0x50414D54;
    pub const TRKS: u32 = 0x534B5254;
}

pub struct WozTrack {
    pub bit_count: u32,
    pub data: Vec<u8>,
}

pub struct WozImage {
    pub write_protected: bool,
    pub tracks: Vec<WozTrack>,
}

// Data is stored in image in little-endian format
fn get_bytes_4(buf: &[u8], start: usize) -> u32 {
    u32::from_le_bytes(buf[start..start + 4].try_into().unwrap())
}

fn get_bytes_2(buf: &[u8], start: usize) -> u16 {
    u16::from_le_bytes(buf[start..start + 2].try_into().unwrap())
}

impl WozImage {
    fn verify(file_buf: &[u8]) -> Result<(), &'static str> {
        let signature = get_bytes_4(file_buf, 0);
        let high_bits = file_buf[4];
        let lfcr = get_bytes_4(file_buf, 5) & 0x00FFFFFF;

        if signature == section_id::WOZ2 && high_bits == 0xFF && lfcr == 0x0A0D0A {
            Ok(())
        } else {
            Err("File is not a WOZ2 disk image.")
        }
    }

    fn parse_info(file_buf: &[u8], buf_pntr: usize) -> Result<bool, &'static str> {
        let version = file_buf[buf_pntr];
        let disk_type = file_buf[buf_pntr + 1];
        let write_protected = file_buf[buf_pntr + 2];
        let boot_sectors = file_buf[buf_pntr + 38];
        let supported = get_bytes_2(file_buf, buf_pntr + 40);
        let compatibile = supported == 0 || supported & 0x3 != 0;
        // Lots of other things we can check in the future...

        if version == 2 && disk_type == 1 && boot_sectors != 2 && compatibile {
            Ok(matches!(write_protected, 1))
        } else {
            Err("This WOZ image is not supported.")
        }
    }

    fn verify_track_map(file_buf: &[u8], buf_pntr: usize) -> Result<(), &'static str> {
        for i in 0..160 {
            let map = file_buf[buf_pntr + i];

            if i >= 140 {
                if map != 0xFF {
                    return Err("WOZ images using more than 35 tracks is not supported.");
                }

                continue;
            }

            if i % 4 == 0 && map != (i / 4) as u8 {
                println!("Map: {map}, {}", i / 4);
                return Err("This WOZ image uses unsupported track mapping.");
            } else if i % 2 == 0 && i % 4 != 0 && map != 0xFF {
                return Err("This WOZ image utilizes odd tracks which is not supported.");
            }
        }

        Ok(())
    }

    fn parse_tracks(file_buf: &[u8], buf_pntr: usize, tracks: &mut Vec<WozTrack>) {
        for i in 0..MAX_TRACKS {
            let offset = buf_pntr + (i * 8);
            let block_addr = get_bytes_2(file_buf, offset) as usize * 512;
            let bit_count = get_bytes_4(file_buf, offset + 4);
            let byte_count = (bit_count as f32 / 8.0).ceil() as usize;
            let mut data: Vec<u8> = Vec::new();

            for j in 0..byte_count {
                data.push(file_buf[block_addr + j]);
            }

            tracks.push(WozTrack { bit_count, data });
        }
    }

    pub fn new(file_path: &Path) -> Result<Self, &'static str> {
        let mut file_buf = [0; WOZ_IMG_SIZE];
        let ext = file_path.extension().unwrap().to_str().unwrap();

        if ext == "woz" {
            let mut image = File::open(file_path).expect("Failed to open WOZ image!");
            image
                .read_exact(&mut file_buf)
                .expect("Failed to read WOZ image data!");
        } else if ext == "dsk" {
            dsk2woz::convert(file_path, &mut file_buf, false);
        } else if ext == "po" {
            dsk2woz::convert(file_path, &mut file_buf, true);
        } else {
            return Err("Unsupported disk image type.");
        }

        WozImage::verify(&file_buf)?;

        let mut write_protected = false;
        let mut tracks = Vec::new();
        let mut buf_pntr: usize = 12;

        loop {
            let chunk_id = get_bytes_4(&file_buf, buf_pntr);
            let chunk_size = get_bytes_4(&file_buf, buf_pntr + 4);
            buf_pntr += 8;

            match chunk_id {
                section_id::INFO => {
                    write_protected = WozImage::parse_info(&file_buf, buf_pntr)?;
                }
                section_id::TMAP => {
                    WozImage::verify_track_map(&file_buf, buf_pntr)?;
                }
                section_id::TRKS => {
                    WozImage::parse_tracks(&file_buf, buf_pntr, &mut tracks);
                }
                _ => {
                    break; // Unknown chunk, so stop
                }
            }

            buf_pntr += chunk_size as usize;
        }

        Ok(WozImage {
            write_protected,
            tracks,
        })
    }
}
