/// Parse a CIDR address string (e.g. "10.0.0.5/24") into (ip, netmask).
/// Falls back to (addr, "255.255.255.0") if the prefix is missing or invalid.
pub(crate) fn parse_cidr(cidr: &str) -> (String, String) {
    if let Some((ip, prefix)) = cidr.split_once('/') {
        let mask = prefix_to_mask(prefix.parse::<u8>().unwrap_or(24));
        (ip.to_string(), mask)
    } else {
        (cidr.to_string(), "255.255.255.0".to_string())
    }
}

fn prefix_to_mask(prefix: u8) -> String {
    let bits = if prefix >= 32 {
        0xFFFF_FFFFu32
    } else {
        !(0xFFFF_FFFFu32 >> prefix)
    };
    format!(
        "{}.{}.{}.{}",
        (bits >> 24) & 0xFF,
        (bits >> 16) & 0xFF,
        (bits >> 8) & 0xFF,
        bits & 0xFF
    )
}
