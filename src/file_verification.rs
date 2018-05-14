// Checksuming files and friends

use std;
use std::fmt;

use std::io::Read;
use std::io::Write;
use std::io::BufRead;
use std::io::BufReader;
use std::io::ErrorKind;

use std::fs;
use std::fs::File;

use std::path::{Path}; // , PathBuf};

extern crate crc;
use self::crc::{crc32, Hasher32}; // https://docs.rs/crc/1.7.0/crc/index.html

extern crate md5;

pub struct ChecksumEntry {
    pub filename: String,
    pub path: String,
    pub checksum_crc32: Option<u32>,
    pub checksum_md5: Option<[u8; 16]>,
    pub valid: bool
}

impl ChecksumEntry {
    pub fn checksum_md5_as_str(&self) -> String {
        let mut s = String::new();
        for &byte in self.checksum_md5.unwrap().iter() {
            s += &format!("{:02x}", byte);
        }
        s    
    }    
}

impl fmt::Debug for ChecksumEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ChecksumEntry(filename:{:?}|path:{:?}|checksum_crc32:{:?}|valid:{:?})", 
            self.filename,
            self.path,
            self.checksum_crc32,
            self.valid,
        )
    }
}

pub fn get_checksum_from_file(file: &String, print_progress: bool) -> Result<ChecksumEntry, std::io::Error> {
    let mut digest_crc32 = crc32::Digest::new(crc32::IEEE);
    let mut context_md5 = md5::Context::new();

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

        digest_crc32.write(&buffer[0..read_count]);
        context_md5.consume(&buffer[0..read_count]);

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

    let digest_md5 = context_md5.compute();

    let file_path = Path::new(file);
    let path = String::from(file_path.to_str().unwrap());
    let filename = String::from(file_path.file_name().unwrap().to_str().unwrap());

    let entry = ChecksumEntry {
        filename: filename,
        path: path,
        checksum_crc32: Some(digest_crc32.sum32()),
        checksum_md5: Some(digest_md5.0),
        valid: true,
    };

    Ok(entry)
}

pub enum SourceTypes {
    SFV,
    PAR2
}

pub struct ChecksumCatalogFile {
    pub entries: Vec<ChecksumEntry>,
    pub valid: bool,
    pub complete: bool,
    pub source_type: SourceTypes,
}

impl fmt::Debug for ChecksumCatalogFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ChecksumCatalogFile(entries:{:?}|valid:{:?})", 
            self.entries,
            self.valid,
        )
    }
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

pub fn read_sfv(filepath: &String) -> Result<ChecksumCatalogFile, std::io::Error> {
    return _read_sfv(filepath, false);
}

fn _read_sfv(filepath: &String, check_only: bool) -> Result<ChecksumCatalogFile, std::io::Error> {
    let mut sfv_file = ChecksumCatalogFile {
        valid: true,
        entries: Vec::new(),
        complete: false,
        source_type: SourceTypes::SFV,
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
        checksum_crc32: None,
        checksum_md5: None,
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
    entry.checksum_crc32 = Some(u32::from_str_radix(&checksum, 16).unwrap());

    Some(entry)
}
