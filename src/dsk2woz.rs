/*
Converts a DSK image into WOZ2 format
Heavily inspired by: https://github.com/mr-stivo/dsk2woz2/blob/master/dsk2woz2.c
Essentially a rewrite in Rust

WOZ Reference: https://applesaucefdc.com/woz/reference2/
*/

use std::path::Path;
use std::{fs::File, io::Read};

const DSK_IMG_SIZE: usize = 143360;

const NUM_TRACKS: u32 = 35;
const BLOCK_SIZE: u32 = 512;
const BLOCKS_PER_TRACK: u32 = 13;
const BITS_PER_TRACK: u32 = 50304;

const NUM_SECTORS: u32 = 16;
const BYTES_PER_SECTOR: u32 = 256;
const GCR_BYTES_PER_SECTOR: u32 = 343;

mod section_id {
    pub const WOZ2: u32 = 0x325A4F57;
    pub const INFO: u32 = 0x4F464E49;
    pub const TMAP: u32 = 0x50414D54;
    pub const TRKS: u32 = 0x534B5254;
}

fn put_u32(value: u32, woz: &mut [u8], start: usize) {
    let bytes = value.to_le_bytes();
    woz[start..start + 4].copy_from_slice(&bytes);
}

fn put_u16(value: u16, woz: &mut [u8], start: usize) {
    let bytes = value.to_le_bytes();
    woz[start..start + 2].copy_from_slice(&bytes);
}

fn fill_header(woz: &mut [u8]) {
    put_u32(section_id::WOZ2, woz, 0);
    woz[4] = 0xFF;
    put_u32(0x000A0D0A, woz, 5);
    put_u32(0x00000000, woz, 8);
}

fn fill_info(woz: &mut [u8]) {
    put_u32(section_id::INFO, woz, 12);
    put_u32(60, woz, 16);
    woz[20] = 2;
    woz[21] = 1;
    woz[22] = 0;
    woz[23] = 0;
    woz[24] = 1;
    woz[24..57].fill(0x20);
    woz[57] = 1;
    woz[58] = 0;
    woz[59] = 32;
    put_u16(0, woz, 60);
    put_u16(0, woz, 62);
    put_u16(13, woz, 64);
}

fn fill_tmap(woz: &mut [u8]) {
    put_u32(section_id::TMAP, woz, 80);
    put_u32(160, woz, 84);

    woz[88..90].fill(0);
    woz[90..248].fill(0xFF);
    for i in 1..NUM_TRACKS as usize {
        let idx = 88 + (i * 4);
        woz[idx - 1..idx + 2].fill(i as u8);
    }
}

fn fill_trks(woz: &mut [u8], file_buf: &[u8]) {
    put_u32(section_id::TRKS, woz, 248);
    put_u32(1280 + (BLOCK_SIZE * BLOCKS_PER_TRACK * NUM_TRACKS), woz, 252);

    for i in 0..NUM_TRACKS {
        let idx = (BYTES_PER_SECTOR + (i * 8)) as usize;
        put_u16(3 + (i as u16 * BLOCKS_PER_TRACK as u16), woz, idx);
        put_u16(BLOCKS_PER_TRACK as u16, woz, idx + 2);
        put_u16(BITS_PER_TRACK as u16, woz, idx + 4);
    }

    let mut woz_idx = 0x600; // Start address of first track
    for i in 0..NUM_TRACKS {
        let dsk_idx = (BYTES_PER_SECTOR * NUM_SECTORS * i) as usize;
        convert_track(&mut woz[woz_idx..], &file_buf[dsk_idx..], i as u8);
        woz_idx += (BLOCK_SIZE * BLOCKS_PER_TRACK) as usize;
    }
}

fn write_bit(woz: &mut [u8], bit_pntr: &mut usize, bit: u8) {
    let byte_idx = *bit_pntr as usize / 8;
    let bit_on = *bit_pntr % 8;
    woz[byte_idx] |= bit << (7 - bit_on);
    *bit_pntr += 1;
}

fn write_byte(woz: &mut [u8], bit_pntr: &mut usize, mut byte: u8) {
    for _ in 0..8 {
        let bit = byte >> 7;
        byte <<= 1;
        write_bit(woz, bit_pntr, bit);
    }
}

fn write_4_4(woz: &mut [u8], bit_pntr: &mut usize, byte: u8) {
    // Split byte into its even and odd bits and write as two separate bytes
    write_byte(woz, bit_pntr, (byte >> 1) | 0xAA);
    write_byte(woz, bit_pntr, byte | 0xAA);
}

fn write_sync(woz: &mut [u8], bit_pntr: &mut usize) {
    write_byte(woz, bit_pntr, 0xFF);
    *bit_pntr += 2; // To account for trailing zeroes of this 10-bit byte
}

fn convert_6_2(dsk: &[u8]) -> [u8; GCR_BYTES_PER_SECTOR as usize] {
    let map = [
        0x96, 0x97, 0x9A, 0x9B, 0x9D, 0x9E, 0x9F, 0xA6,
        0xA7, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF, 0xB2, 0xB3,
        0xB4, 0xB5, 0xB6, 0xB7, 0xB9, 0xBA, 0xBB, 0xBC,
        0xBD, 0xBE, 0xBF, 0xCB, 0xCD, 0xCE, 0xCF, 0xD3,
        0xD6, 0xD7, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE,
        0xDF, 0xE5, 0xE6, 0xE7, 0xE9, 0xEA, 0xEB, 0xEC,
        0xED, 0xEE, 0xEF, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6,
        0xF7, 0xF9, 0xFa, 0xFB, 0xFC, 0xFD, 0xFE, 0xFF
    ];

    let mut gcr_bytes = [0; GCR_BYTES_PER_SECTOR as usize];

    let bit_reverse = [0, 2, 1, 3];
    for i in 0..84 {
        gcr_bytes[i] = bit_reverse[(dsk[i] & 3) as usize] |
                     (bit_reverse[(dsk[i + 86] & 3) as usize] << 2) |
                     (bit_reverse[(dsk[i + 172] & 3) as usize] << 4);
    }
    gcr_bytes[84] = bit_reverse[(dsk[84] & 3) as usize] |
                  (bit_reverse[(dsk[170] & 3) as usize] << 2);
    gcr_bytes[85] = bit_reverse[(dsk[85] & 3) as usize] |
                  (bit_reverse[(dsk[171] & 3) as usize] << 2);

    for i in 0..BYTES_PER_SECTOR as usize {
        gcr_bytes[86 + i] = dsk[i] >> 2;
    }
    
    let mut idx = (GCR_BYTES_PER_SECTOR - 1) as usize;
    gcr_bytes[idx] = gcr_bytes[idx - 1];
    while idx > 1 {
        idx -= 1;
        gcr_bytes[idx] ^= gcr_bytes[idx - 1];
    }

    for i in 0..GCR_BYTES_PER_SECTOR as usize {
        gcr_bytes[i] = map[gcr_bytes[i] as usize];
    }

    gcr_bytes
}

fn convert_track(woz: &mut [u8], dsk: &[u8], track: u8) {
    let mut bit_pntr = 0;

    // Gap 1
    for _ in 0..16 {
        write_sync(woz, &mut bit_pntr);
    }

    for i in 0..NUM_SECTORS as u8 {
        // Address Prologue
        write_byte(woz, &mut bit_pntr, 0xD5);
        write_byte(woz, &mut bit_pntr, 0xAA);
        write_byte(woz, &mut bit_pntr, 0x96);

        // Volume, track, sector, and checksum
        write_4_4(woz, &mut bit_pntr, 254);
        write_4_4(woz, &mut bit_pntr, track);
        write_4_4(woz, &mut bit_pntr, i);
        write_4_4(woz, &mut bit_pntr, 254 ^ track ^ i);

        // Address Epilogue
        write_byte(woz, &mut bit_pntr, 0xDE);
        write_byte(woz, &mut bit_pntr, 0xAA);
        write_byte(woz, &mut bit_pntr, 0xEB);

        // Gap 2
        for _ in 0..7 {
            write_sync(woz, &mut bit_pntr);
        }

        // Data Prologue
        write_byte(woz, &mut bit_pntr, 0xD5);
        write_byte(woz, &mut bit_pntr, 0xAA);
        write_byte(woz, &mut bit_pntr, 0xAD);

        let logical_sector = match i == 15 {
            true => 15,
            false => (i as usize * 7) % 15
        };

        // Convert 256 data bytes into 343 6 and 2 encoded disk bytes
        let gcr_bytes = convert_6_2(&dsk[logical_sector * BYTES_PER_SECTOR as usize..]);
        for b in gcr_bytes.into_iter() {
            write_byte(woz, &mut bit_pntr, b);
        }

        // Data Epilogue
        write_byte(woz, &mut bit_pntr, 0xDE);
        write_byte(woz, &mut bit_pntr, 0xAA);
        write_byte(woz, &mut bit_pntr, 0xEB);

        // Gap 3
        for _ in 0..16 {
            write_sync(woz, &mut bit_pntr);
        }
    }
}

pub fn convert(file_path: &Path, woz: &mut [u8]) {
    let mut file_buf = [0; DSK_IMG_SIZE];
    let mut image = File::open(file_path).expect("Failed to open DSK image!");
    image.read(&mut file_buf).expect("Failed to read DSK image data!");

    fill_header(woz);
    fill_info(woz);
    fill_tmap(woz);
    fill_trks(woz, &file_buf);
}