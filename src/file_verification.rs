// Checksuming files and friends

extern crate crc;

use std;
use std::io::Read;
use std::io::Write;
use std::io::BufRead;
use std::io::BufReader;
use std::io::ErrorKind;

use std::fs;
use std::fs::File;

use self::crc::{crc32, Hasher32}; // https://docs.rs/crc/1.7.0/crc/index.html

pub struct ChecksumEntry {
    pub filename: String,
    pub path: String,
    pub checksum_crc32: u32,
    pub valid: bool
}

pub fn get_crc32_from_file(file: &String, print_progress: bool) -> Result<u32, std::io::Error> {
    let mut digest = crc32::Digest::new(crc32::IEEE);

    let mut f = match File::open(file) {
        Ok(v) => v,
        Err(_e) => return Err(_e)
    };

    const BUFFER_SIZE: usize = 1024*1024;
    let mut buffer = [0u8; BUFFER_SIZE];
    let mut read_count;
    let mut read_pos: u64 = 0;
    let mut read_perc = 0;
    let prog_bar_size = 50;

    let fmeta = fs::metadata(file).unwrap();
    let read_max = fmeta.len();

    loop {
        read_count = f.read(&mut buffer).unwrap();
        read_pos += read_count as u64;
        if read_count == 0 { break; }
        digest.write(&buffer[0..read_count]);

        if print_progress {
            let perc = prog_bar_size * read_pos / read_max;
            if perc > read_perc {
                print!("#");
                std::io::stdout().flush().ok();
                read_perc = perc;
            }
        }
    }
    if print_progress {
        let packets = read_pos / buffer.len() as u64;
        let missing: i64 = prog_bar_size as i64 - packets as i64;

        if missing > 0 {
            for _i in 0..missing {
                print!("#");
            }
        }
        println!(" {} bytes read", read_pos);
    }

    Ok(digest.sum32())
}

pub struct SfvFile {
    pub valid: bool,
    pub entries: Vec<ChecksumEntry>,
}

pub fn is_sfv(filepath: &String) -> bool {
    let res = _read_sfv(filepath, true);
    if res.is_ok() {
        let sfv = res.unwrap();
        if sfv.entries.len() == 0 {
            return false;
        } else {
            match sfv.entries.first() {
                Some(e) => return e.valid,
                None    => return false,
            }
        }
    }
    false
}

pub fn read_sfv(filepath: &String) -> Result<SfvFile, std::io::Error> {
    return _read_sfv(filepath, false);
}

fn _read_sfv(filepath: &String, check_only: bool) -> Result<SfvFile, std::io::Error> {
    let mut sfv_file = SfvFile {
        valid: true,
        entries: Vec::new(),
    };

    let fres: Result<File, std::io::Error> = match File::open(filepath) {
        Ok(v) => Ok(v),
        Err(_e) => return Err(_e)
    };

    let fh = fres.unwrap();
    let file = BufReader::new(&fh);
    for rline in file.lines() {
        if rline.is_ok() {
            let line = rline.unwrap();
            let entry = parse_sfv_line(&line);
            if entry.is_some() {
                sfv_file.entries.push(entry.unwrap());
                if check_only {
                    return Ok(sfv_file);
                }
            }
        } else {
            return Err(std::io::Error::new(ErrorKind::Other, rline.err().unwrap()));
        }
    }
    Ok(sfv_file)
}

pub fn parse_sfv_line(line_par: &String) -> Option<ChecksumEntry> {
    let mut entry = ChecksumEntry {
        filename: String::new(),
        path: String::new(),
        checksum_crc32: 0,
        valid: true,
    };

    let line = line_par.trim();
    let num_chars = line.chars().count();

    if line.starts_with(';') {
        return None;
    }

    let mut checksum = String::new();

    let mut i = 0;

    for c in line.chars().rev() {
        if c == ' ' || c == '\t' {
            if i != 8 {
                entry.valid = false;
                return Some(entry);                
            }
            break;
        } else {
            if c >= '0' && c <= '9' || c >= 'a' && c <= 'f' || c >= 'A' && c <= 'F' {
                checksum.push(c);
            } else {
                entry.valid = false;
                return Some(entry);
            }
        }
        i += 1;
    }

    checksum = checksum.chars().rev().collect::<String>();

    entry.filename = line_par.chars().take(num_chars-i-1).collect::<String>();
    entry.checksum_crc32 = u32::from_str_radix(&checksum, 16).unwrap();

    Some(entry)
}
