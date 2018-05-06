

pub fn printable_string_from(buffer: &[u8]) -> String {
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

pub fn byte_array_to_hex(bytes: &[u8]) -> String {
    let mut s = String::new();
    for &byte in bytes {
        if s.len() > 0 { s += "" }
        s += &format!("{:02x}", byte);
        // write!(&mut s, "{:X} ", byte).expect("Unable to write");
    }
    s    
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