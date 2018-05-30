
use std;
use std::fmt;
use std::vec::Vec;

use std::io::Read;
use std::io::{Seek, SeekFrom};

use std::fs::File;

use std::ops::Fn;

use utils;
use file_verification;

pub const EXTENSION: &str = "par2";

const PAR2_MAGIC: &[u8;8] = b"PAR2\0PKT";

const PAR2_PKT_TYPE_FILE_DESC: &[u8;16] = b"PAR 2.0\0FileDesc";
const PAR2_PKT_TYPE_IFSC: &[u8;16] = b"PAR 2.0\0IFSC\0\0\0\0";
const PAR2_PKT_TYPE_MAIN: &[u8;16] = b"PAR 2.0\0Main\0\0\0\0";
const PAR2_PKT_TYPE_CREATOR: &[u8;16] = b"PAR 2.0\0Creator\0";

static mut VERBOSE: bool = false;
pub fn set_verbose(is: bool) { unsafe { VERBOSE = is; } }
pub fn is_verbose() -> bool  { unsafe { return VERBOSE; } }
pub fn if_verbose(func: &Fn()) { if is_verbose() { func(); } }

enum Par2PacketTypes {
    Unknown,
    Main(Par2MainPacket),
    Creator(Par2CreatorPacket),
    FileDescriptor(Par2FileDescriptorPacket),
    InputFileSliceChecksum(Par2InputFileSliceChecksumPacket),
}

impl Default for Par2PacketTypes {
    fn default() -> Par2PacketTypes { Par2PacketTypes::Unknown }
}

impl fmt::Debug for Par2PacketTypes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Par2PacketTypes::Unknown => {
                write!(f, "Unknown")
            },
            &Par2PacketTypes::Main(ref pkt) => {
                write!(f, "Main({:?})", pkt)
            },
            &Par2PacketTypes::Creator(ref pkt) => {
                write!(f, "Creator({:?})", pkt)
            },
            &Par2PacketTypes::FileDescriptor(ref pkt) => {
                write!(f, "FileDescriptor({:?})", pkt)
            },            
            &Par2PacketTypes::InputFileSliceChecksum(ref pkt) => {
                write!(f, "InputFileSliceChecksum({:?})", pkt)
            },
        }
    }
}

const HEAD_LEN_I64: i64 = 8+8+16+16+16;
const HEAD_LEN: usize = 8+8+16+16+16;

#[derive(Default)]
struct Par2PacketHead {
    pub magic: [u8;8],
    pub len: u64,
    pub packet_hash: [u8;16],
    pub recovery_set_id: [u8;16],
    pub packet_type: [u8;16],
    pub packet_body: Par2PacketTypes,
}

#[derive(Default)] #[derive(Debug)]
struct Par2MainPacket {
    pub slice_size: u64,
    pub number_of_files: u32,
    // ?*16	MD5 Hash array	File IDs of all files in the recovery set
    // ?*16	MD5 Hash array	File IDs of all files in the non-recovery set
}

#[derive(Default)]
struct Par2CreatorPacket {
    pub client_identifier: String,
}

impl fmt::Debug for Par2CreatorPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Par2CreatorPacket(client:'{}')", 
            &self.client_identifier,
        )
    }
}

#[derive(Default)]
struct Par2FileDescriptorPacket {
    pub file_id: [u8;16],
    pub entire_file_md5: [u8;16],
    pub first_16k_md5: [u8;16],
    pub length_of_file: u64,
    pub name_of_file: String,
    // ?*4	ASCII char array	Name of the file
}

impl fmt::Debug for Par2FileDescriptorPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Par2FileDescriptorPacket {{ file_id: {}, entire_file_md5: {}, first_16k_md5: {}, length_of_file: {:?}, name_of_file: '{}' }}", 
            utils::byte_array_to_hex(&self.file_id),
            utils::byte_array_to_hex(&self.entire_file_md5),
            utils::byte_array_to_hex(&self.first_16k_md5),
            self.length_of_file,
            self.name_of_file,
        )
    }
}

#[derive(Default)]
struct Par2InputFileSliceChecksumPacket {
    pub file_id: [u8;16],
    // ?*20	{MD5 Hash, CRC32} MD5 Hash and CRC32 pairs for the slices
}

impl fmt::Debug for Par2InputFileSliceChecksumPacket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Par2InputFileSliceChecksumPacket {{ file_id: {} }}", 
            utils::byte_array_to_hex(&self.file_id),
        )
    }
}

impl fmt::Debug for Par2PacketHead {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Par2PacketHead {{ magic: '{}', len: {:?}, packet_hash: {}, recovery_set_id: {}, packet_type: '{}' }}", 
            utils::printable_string_from(&self.magic),
            self.len,
            utils::byte_array_to_hex(&self.packet_hash),
            utils::byte_array_to_hex(&self.recovery_set_id),
            utils::printable_string_from(&self.packet_type), // utils::byte_array_to_hex(&self.packet_type),
        )
    }
}

pub fn is_par2(filepath: &String) -> bool {
    let res = _read_par2(filepath, true);
    if res.is_ok() {
        return res.unwrap().valid;
    }
    false    
}

pub fn read_par2(filepath: &String) -> Result<file_verification::ChecksumCatalog, std::io::Error> {
    return _read_par2(filepath, false);
}

fn _read_par2(filepath: &String, _check_only: bool) -> Result<file_verification::ChecksumCatalog, std::io::Error> {
    let mut catalog_file = file_verification::ChecksumCatalog {
        valid: true,
        entries: Vec::new(),
        complete: false,
        source_type: file_verification::SourceTypes::PAR2,
        source_file: filepath.to_string(),
        state: 0,
    };

    let fres: Result<File, std::io::Error> = match File::open(filepath) {
        Ok(v) => Ok(v),
        Err(_e) => return Err(_e)
    };

    let mut buf_head: [u8; HEAD_LEN] = [0; HEAD_LEN];
    let mut file_ids: Vec<[u8;16]> = Vec::new();

    if fres.is_ok() {
        let mut fh = fres.unwrap();

        loop {
            let bytes = fh.read(&mut buf_head)?;
            if bytes == 0 { break; }
            if bytes < HEAD_LEN_I64 as usize {
                    catalog_file.valid = false;
                    break;
            }
            let head = _parse_par2_packet_head(&buf_head[0..bytes]);

            if head.is_some() {
                let mut head = head.unwrap();
                if_verbose(&|| println!("{:?}", head));

                if &head.magic != PAR2_MAGIC {
                    catalog_file.valid = false;
                    break;
                } else if _check_only {
                    catalog_file.valid = true;
                    break;
                }

                head.packet_body = _parse_par2_packet_body(&head, &mut fh).unwrap();

                if let Par2PacketTypes::Unknown = head.packet_body {
                    // NOP
                } else if let Par2PacketTypes::FileDescriptor(_body) = head.packet_body {
                    let entry = file_verification::ChecksumEntry {
                        filename: _body.name_of_file.to_string(),
                        path: String::new(),
                        checksum_crc32: None,
                        checksum_md5: Some(_body.entire_file_md5),
                        valid: true,
                        state: 0,
                    };

                    if !file_ids.contains(&_body.file_id) {
                        file_ids.push(_body.file_id);
                        catalog_file.entries.push(entry);
                    }

                    if_verbose(&|| println!("{:?}", _body));

                } else {
                    if_verbose(&|| println!("{:?}", head.packet_body));
                } 

            } else {
                catalog_file.valid = false;
                break;
            }
        }
    }

    Ok(catalog_file)
}

fn _parse_par2_packet_head(buffer: &[u8]) -> Option<Par2PacketHead> {
    let mut head = Par2PacketHead {
        magic: Default::default(), // 0;8
        len: 0, // 8;8
        packet_hash: Default::default(), // 16:16
        recovery_set_id: Default::default(), // 32:16
        packet_type: Default::default(), // 48:16
        packet_body: Default::default(),
    };

    // println!("buffer.len={:?} head.magic={:?} buffer[0..7].len={:?}", buffer.len(), head.magic, buffer[0..7].len());

    head.magic.copy_from_slice(&buffer[0..8]);
    head.len = utils::slice_u8_to_u64(&buffer[8..16]);
    head.packet_hash.copy_from_slice(&buffer[16..32]);
    head.recovery_set_id.copy_from_slice(&buffer[32..48]);
    head.packet_type.copy_from_slice(&buffer[48..64]);

    Some(head)
}

fn _parse_par2_packet_body(head: &Par2PacketHead, mut fh: &File) -> Option<Par2PacketTypes> {

    let to_skip = 0i64 + head.len as i64 - HEAD_LEN_I64;

    let packet_type = match &head.packet_type {
        
        PAR2_PKT_TYPE_CREATOR => {
            let mut body = Par2CreatorPacket {
                client_identifier: String::new(),
            };
            &fh.take(to_skip as u64).read_to_string(&mut body.client_identifier);

            Par2PacketTypes::Creator(body)
        },

        PAR2_PKT_TYPE_MAIN => {
            let mut buffer_vec: Vec<u8> = vec![0;to_skip as usize];

            buffer_vec.reserve_exact(to_skip as usize);
            let mut buffer = buffer_vec.as_mut_slice();

            &fh.take(to_skip as u64).read(buffer);

            let mut body = Par2MainPacket {
                slice_size: utils::slice_u8_to_u64(&buffer[0..8]),
                number_of_files: utils::slice_u8_to_u32(&buffer[8..12]),
            };

            Par2PacketTypes::Main(body)
        },
        
        PAR2_PKT_TYPE_IFSC => {
            let mut buffer_vec: Vec<u8> = vec![0;to_skip as usize];
            
            buffer_vec.reserve_exact(to_skip as usize);
            let mut buffer = buffer_vec.as_mut_slice();

            &fh.take(to_skip as u64).read(buffer);

            let mut body = Par2InputFileSliceChecksumPacket {
                file_id: Default::default()
            };
            body.file_id.copy_from_slice(&buffer[0..16]);
            Par2PacketTypes::InputFileSliceChecksum(body)
        },
        
        PAR2_PKT_TYPE_FILE_DESC => {
            let mut buffer_vec: Vec<u8> = vec![0;to_skip as usize];

            buffer_vec.reserve_exact(to_skip as usize);
            let mut buffer = buffer_vec.as_mut_slice();

            &fh.take(to_skip as u64).read(buffer);

            let filename = String::from_utf8(buffer[56..(to_skip-1) as usize].to_vec()).unwrap()
                            .trim_right_matches('\u{0}')
                            .to_string();
            // filename.retain(|c| c != '\u{0}');

            let mut body = Par2FileDescriptorPacket {
                file_id: Default::default(),
                entire_file_md5: Default::default(),
                first_16k_md5: Default::default(),
                length_of_file: utils::slice_u8_to_u64(&buffer[48..56]),
                name_of_file: filename
                ,
            };
            body.file_id.copy_from_slice(&buffer[0..16]);
            body.entire_file_md5.copy_from_slice(&buffer[16..32]);
            body.first_16k_md5.copy_from_slice(&buffer[32..48]);

            Par2PacketTypes::FileDescriptor(body)
        },
        
        _ => Par2PacketTypes::Unknown,
    };

    if let Par2PacketTypes::Unknown = packet_type {
        fh.seek(SeekFrom::Current(to_skip)).unwrap();
    }
    
    Some(packet_type)
}
