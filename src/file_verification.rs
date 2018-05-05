// Checksuming files and friends

extern crate crc;

use std;
use std::fmt;

use std::io::Read;
use std::io::Write;
use std::io::{Seek, SeekFrom};
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

pub struct ChecksumCatalogFile {
    pub entries: Vec<ChecksumEntry>,
    pub valid: bool,
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

fn printable_string_from(buffer: &[u8]) -> String {
    let mut ptbls = String::new();
    for c in buffer.iter().cloned() {
        if c >= 32u8 && c <= 126u8 {
            ptbls.push(c as char);
        } else {
            ptbls.push(' ');
        }
    }
    ptbls
}

fn byte_array_to_hex(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &byte in bytes {
        if s.len() > 0 { s += " " }
        s += &format!("{:02X}", byte);
        // write!(&mut s, "{:X} ", byte).expect("Unable to write");
    }
    s    
}

#[derive(Default)]
struct Par2PacketHead {
    pub magic: [u8;8],
    pub len: u64,
    pub packet_hash: [u8;16],
    pub recovery_set_id: [u8;16],
    pub packet_type: [u8;16],
}

impl fmt::Debug for Par2PacketHead {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Par2PacketHead(magic:[{}]|len:{:?}|packet_hash:[{}]|recovery_set_id:[{}]|packet_type:[{}] '{}')", 
            byte_array_to_hex(&self.magic),
            self.len,
            byte_array_to_hex(&self.packet_hash),
            byte_array_to_hex(&self.recovery_set_id),
            byte_array_to_hex(&self.packet_type),
            printable_string_from(&self.packet_type),
        )
    }
}

pub fn read_par2(filepath: &String) -> Result<ChecksumCatalogFile, std::io::Error> {
    return _read_par2(filepath, false);
}

fn _read_par2(filepath: &String, _check_only: bool) -> Result<ChecksumCatalogFile, std::io::Error> {
    let sfv_file = ChecksumCatalogFile {
        valid: true,
        entries: Vec::new(),
    };

    let fres: Result<File, std::io::Error> = match File::open(filepath) {
        Ok(v) => Ok(v),
        Err(_e) => return Err(_e)
    };

    const HEAD_LEN_I64: i64 = 8+8+16+16+16;
    const HEAD_LEN: usize = 8+8+16+16+16;
    let mut buf_head: [u8; HEAD_LEN] = [0; HEAD_LEN];

    if fres.is_ok() {
        let mut fh = fres.unwrap();

        loop {
            let bytes = fh.read(&mut buf_head)?;
            if bytes == 0 { break; }
            let head = _parse_par2_packet_head(&buf_head[0..bytes]);

            if head.is_some() {
                let head = head.unwrap();
                println!("{:?}", head);

                let to_skip = 0i64 + head.len as i64 - HEAD_LEN_I64;

                fh.seek(SeekFrom::Current(to_skip)).unwrap();

            } else {
                break;
            }
        }
    }

    Ok(sfv_file)
}

fn _parse_par2_packet_head(buffer: &[u8]) -> Option<Par2PacketHead> {
    let mut head = Par2PacketHead {
        magic: Default::default(), // 0;8
        len: 0, // 8;8
        packet_hash: Default::default(), // 16:16
        recovery_set_id: Default::default(), // 32:16
        packet_type: Default::default(), // 48:16
    };

    // println!("buffer.len={:?} head.magic={:?} buffer[0..7].len={:?}", buffer.len(), head.magic, buffer[0..7].len());

    head.magic.copy_from_slice(&buffer[0..8]);
    head.len = 0;
    for i in 0..7 {
        // println!("{}: {}", i, buffer[8+i]);
        head.len += (buffer[8+i] as u64) << (i*8);
    }
    head.packet_hash.copy_from_slice(&buffer[16..32]);
    head.recovery_set_id.copy_from_slice(&buffer[32..48]);
    head.packet_type.copy_from_slice(&buffer[48..64]);

    Some(head)
}