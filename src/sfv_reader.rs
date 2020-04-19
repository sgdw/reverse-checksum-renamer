// reverse-checksum-renamer
// 
// Copyright (C) 2020  Martin Feil aka. SGDW
// 
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
// 
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
// 
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>

use std;
use std::vec::Vec;

use std::io::BufRead;
use std::io::BufReader;
use std::io::ErrorKind;

use std::fs::File;

use file_verification;

pub const EXTENSION: &str = "sfv";

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

pub fn read_sfv(filepath: &String) -> Result<file_verification::ChecksumCatalog, std::io::Error> {
    return _read_sfv(filepath, false);
}

fn _read_sfv(filepath: &String, check_only: bool) -> Result<file_verification::ChecksumCatalog, std::io::Error> {
    let mut catalog_file = file_verification::ChecksumCatalog {
        valid: true,
        entries: Vec::new(),
        complete: false,
        source_type: file_verification::SourceTypes::SFV,
        source_file: filepath.to_string(),
        state: 0,
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
                catalog_file.entries.push(entry.unwrap());
                if check_only {
                    return Ok(catalog_file);
                }
            }
        } else {
            return Err(std::io::Error::new(ErrorKind::Other, rline.err().unwrap()));
        }
    }
    Ok(catalog_file)
}

pub fn parse_sfv_line(line_par: &String) -> Option<file_verification::ChecksumEntry> {
    let mut entry = file_verification::ChecksumEntry {
        filename: String::new(),
        path: String::new(),
        checksum_crc32: None,
        checksum_md5: None,
        valid: true,
        state: 0,
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

    // println!("num_chars:{} i:{}", num_chars, i);
    if num_chars > i && num_chars > 0 && i > 0 {
        entry.filename = line_par.chars().take(num_chars-i-1).collect::<String>();
        entry.checksum_crc32 = Some(u32::from_str_radix(&checksum, 16).unwrap());
    }

    Some(entry)
}