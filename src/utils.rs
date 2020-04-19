

pub fn printable_string_from(buffer: &[u8]) -> String {
    let mut str = String::new();
    for c in buffer.iter().cloned() {
        if c >= 32u8 && c <= 126u8 {
            str.push(c as char);
        } else {
            str.push(' ');
        }
    }
    str
}

pub fn byte_array_to_hex(bytes: &[u8]) -> String {
    let mut str = String::new();
    for &byte in bytes {
        if str.len() > 0 { str += "" }
        str += &format!("{:02x}", byte);
        // write!(&mut s, "{:X} ", byte).expect("Unable to write");
    }
    str    
}

pub fn slice_u8_to_u64(buffer: &[u8]) -> u64 {
    let mut val: u64 = 0;
    for i in 0..7 {
        val += (buffer[i] as u64) << (i*8);
    }
    val
}

pub fn slice_u8_to_u32(buffer: &[u8]) -> u32 {
    let mut val: u32 = 0;
    for i in 0..3 {
        val += (buffer[i] as u32) << (i*8);
    }
    val
}