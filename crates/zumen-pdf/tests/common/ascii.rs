// 合成入力を ASCIIHex・ASCII85 に包む。値は hayro-syntax の展開と照らす。

#[cfg(test)]
pub fn encode(data: &[u8], base85: bool) -> Vec<u8> {
    if !base85 {
        let mut out: String = data.iter().map(|byte| format!("{byte:02x}")).collect();
        out.push('>');
        return out.into_bytes();
    }
    let mut out = Vec::new();
    for bytes in data.chunks(4) {
        let mut group = [0; 4];
        group[..bytes.len()].copy_from_slice(bytes);
        let mut value = u32::from_be_bytes(group);
        if value == 0 && bytes.len() == 4 {
            out.push(b'z');
            continue;
        }
        let mut digits = [0; 5];
        for digit in digits.iter_mut().rev() {
            *digit = (value % 85) as u8 + b'!';
            value /= 85;
        }
        out.extend_from_slice(&digits[..bytes.len() + 1]);
    }
    out.extend(b"~>");
    out
}
