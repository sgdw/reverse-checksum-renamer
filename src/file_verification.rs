// Checksuming files and friends

use std;
use std::fmt;

use std::io::Read;
use std::io::Write;

use std::fs;
use std::fs::File;

use std::path::{Path}; // , PathBuf};

extern crate crc;
use self::crc::{crc32, Hasher32}; // https://docs.rs/crc/1.7.0/crc/index.html

extern crate md5;

use super::par2_reader;
use super::sfv_reader;

#[derive(PartialEq)]
pub enum SourceTypes {
    SFV,
    PAR2
}

pub struct ChecksumCatalog {
    pub entries: Vec<ChecksumEntry>,
    pub valid: bool,
    pub complete: bool,
    pub source_type: SourceTypes,
    pub source_file: String,
    pub state: u64,
}

impl fmt::Debug for ChecksumCatalog {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ChecksumCatalogFile(entries:{:?}|valid:{:?})", 
            self.entries,
            self.valid,
        )
    }
}

pub struct ChecksumEntry {
    pub filename: String,
    pub path: String,
    pub checksum_crc32: Option<u32>,
    pub checksum_md5: Option<[u8; 16]>,
    pub valid: bool,
    pub state: u64,
}

impl ChecksumEntry {
    pub fn checksum_md5_as_str(&self) -> String {
        let mut s = String::new();
        for &byte in self.checksum_md5.unwrap().iter() {
            s += &format!("{:02x}", byte);
        }
        s    
    }

    #[allow(dead_code)]
    pub fn set_state(&mut self, bit: u8) {
        self.state = self.state | 1 << bit;
    }

    #[allow(dead_code)]
    pub fn reset_state(&mut self, bit: u8) {
        self.state = self.state & !(1 << bit);
    }

    #[allow(dead_code)]
    pub fn has_state(&self, bit: u8) -> bool {
        (self.state & (1 << bit)) == (1 << bit)
    }
}

impl fmt::Debug for ChecksumEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ChecksumEntry(filename:{:?}|path:{:?}|checksum_crc32:{:?}|checksum_md5:{:?}|valid:{:?})", 
            self.filename,
            self.path,
            self.checksum_crc32,
            self.checksum_md5_as_str(),
            self.valid,
        )
    }
}

pub fn get_source_type_by_filename(file: &String) -> Option<SourceTypes> {
    let sfv_extension  = ".".to_owned() + sfv_reader::EXTENSION;
    let par2_extension = ".".to_owned() + par2_reader::EXTENSION;

    if file.ends_with(&sfv_extension) {
        return Some(SourceTypes::SFV);
    }
    else if file.ends_with(&par2_extension) {
        return Some(SourceTypes::PAR2);
    }

    None
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
        state: 0,
    };

    Ok(entry)
}