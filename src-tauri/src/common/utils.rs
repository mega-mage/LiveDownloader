pub fn parse_anchor_from_filename(filename: &str) -> String {
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);

    let prefix = if let Some(idx) = find_date_prefix_idx(stem) {
        &stem[..idx]
    } else {
        stem
    };

    let trimmed = prefix.trim_end_matches('_').trim_end_matches('-');
    if trimmed.is_empty() {
        return "Unknown".to_string();
    }

    if let Some(dash_idx) = trimmed.find('-') {
        let name = &trimmed[..dash_idx];
        if !name.is_empty() {
            return name.to_string();
        }
    }

    trimmed.to_string()
}

fn find_date_prefix_idx(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.len() < 11 {
        return None;
    }
    for i in 0..(bytes.len() - 10) {
        if bytes[i] == b'_'
            && bytes[i+1].is_ascii_digit()
            && bytes[i+2].is_ascii_digit()
            && bytes[i+3].is_ascii_digit()
            && bytes[i+4].is_ascii_digit()
            && bytes[i+5] == b'-'
            && bytes[i+6].is_ascii_digit()
            && bytes[i+7].is_ascii_digit()
            && bytes[i+8] == b'-'
            && bytes[i+9].is_ascii_digit()
            && bytes[i+10].is_ascii_digit()
        {
            return Some(i);
        }
    }
    None
}
