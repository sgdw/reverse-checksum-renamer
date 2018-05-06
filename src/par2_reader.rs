
use std;
use std::fmt;

use std::io::Read;
use std::io::{Seek, SeekFrom};
// use std::io::BufRead;
// use std::io::BufReader;
// use std::io::ErrorKind;

use std::fs::File;

use utils;
use file_verification;

const PAR2_PKT_TYPE_FILE_DESC: &[u8;16] = b"PAR 2.0\0FileDesc";
const PAR2_PKT_TYPE_IFSC: &[u8;16] = b"PAR 2.0\0IFSC\0\0\0\0";
const PAR2_PKT_TYPE_MAIN: &[u8;16] = b"PAR 2.0\0Main\0\0\0\0";
const PAR2_PKT_TYPE_CREATOR: &[u8;16] = b"PAR 2.0\0Creator\0";

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

#[derive(Default)]
#[derive(Debug)]
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
#[derive(Debug)]
struct Par2FileDescriptorPacket {
    pub file_id: [u8;16],
    pub entire_file_md5: [u8;16],
    pub first_16k_md5: [u8;16],
    pub length_of_file: u64,
    // ?*4	ASCII char array	Name of the file
}

#[derive(Default)]
#[derive(Debug)]
struct Par2InputFileSliceChecksumPacket {
    pub file_id: [u8;16],
    // ?*20	{MD5 Hash, CRC32} MD5 Hash and CRC32 pairs for the slices
}

impl fmt::Debug for Par2PacketHead {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Par2PacketHead(magic:'{}'|len:{:?}|packet_hash:{}|recovery_set_id:{}|packet_type:{} '{}')", 
            utils::printable_string_from(&self.magic),
            self.len,
            utils::byte_array_to_hex(&self.packet_hash),
            utils::byte_array_to_hex(&self.recovery_set_id),
            utils::byte_array_to_hex(&self.packet_type),
            utils::printable_string_from(&self.packet_type),
        )
    }
}

pub fn read_par2(filepath: &String) -> Result<file_verification::ChecksumCatalogFile, std::io::Error> {
    return _read_par2(filepath, false);
}

fn _read_par2(filepath: &String, _check_only: bool) -> Result<file_verification::ChecksumCatalogFile, std::io::Error> {
    let sfv_file = file_verification::ChecksumCatalogFile {
        valid: true,
        entries: Vec::new(),
    };

    let fres: Result<File, std::io::Error> = match File::open(filepath) {
        Ok(v) => Ok(v),
        Err(_e) => return Err(_e)
    };

    let mut buf_head: [u8; HEAD_LEN] = [0; HEAD_LEN];

    if fres.is_ok() {
        let mut fh = fres.unwrap();

        loop {
            let bytes = fh.read(&mut buf_head)?;
            if bytes == 0 { break; }
            let head = _parse_par2_packet_head(&buf_head[0..bytes]);

            if head.is_some() {
                let mut head = head.unwrap();
                println!("{:?}", head);

                // let to_skip = 0i64 + head.len as i64 - HEAD_LEN_I64;
                // fh.seek(SeekFrom::Current(to_skip)).unwrap();

                head.packet_body = _parse_par2_packet_body(&head, &mut fh).unwrap();

                if let Par2PacketTypes::Unknown = head.packet_body {
                    // NOP
                } else {
                    println!("{:?}", head.packet_body);
                }             

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
        PAR2_PKT_TYPE_MAIN => Par2PacketTypes::Unknown,
        PAR2_PKT_TYPE_IFSC => Par2PacketTypes::Unknown,
        PAR2_PKT_TYPE_FILE_DESC => Par2PacketTypes::Unknown,
        _ => Par2PacketTypes::Unknown,
    };

    if let Par2PacketTypes::Unknown = packet_type {
        fh.seek(SeekFrom::Current(to_skip)).unwrap();
    }
    
    Some(packet_type)
}
