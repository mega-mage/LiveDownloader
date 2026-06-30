use std::time::{SystemTime, UNIX_EPOCH};

fn rc4_encrypt(plaintext: &[u8], key: &[u8]) -> Vec<u8> {
    let mut s: Vec<u8> = (0..=255).collect();
    let mut j: usize = 0;
    for i in 0..256 {
        j = (j + s[i] as usize + key[i % key.len()] as usize) % 256;
        s.swap(i, j);
    }

    let mut i: usize = 0;
    let mut j: usize = 0;
    let mut result = Vec::with_capacity(plaintext.len());
    for &char_byte in plaintext {
        i = (i + 1) % 256;
        j = (j + s[i] as usize) % 256;
        s.swap(i, j);
        let t = (s[i].wrapping_add(s[j])) as usize;
        result.push(s[t] ^ char_byte);
    }
    result
}

fn left_rotate(x: u32, n: u32) -> u32 {
    let n = n % 32;
    if n == 0 {
        x
    } else {
        (x << n) | (x >> (32 - n))
    }
}

fn get_t_j(j: usize) -> u32 {
    if j < 16 { 2043430169 } else { 2055708042 }
}

fn ff_j(j: usize, x: u32, y: u32, z: u32) -> u32 {
    if j < 16 { x ^ y ^ z } else { (x & y) | (x & z) | (y & z) }
}

fn gg_j(j: usize, x: u32, y: u32, z: u32) -> u32 {
    if j < 16 { x ^ y ^ z } else { (x & y) | (!x & z) }
}

struct SM3 {
    reg: [u32; 8],
    chunk: Vec<u8>,
    size: usize,
}

impl SM3 {
    fn new() -> Self {
        let mut sm3 = Self {
            reg: [0; 8],
            chunk: Vec::with_capacity(64),
            size: 0,
        };
        sm3.reset();
        sm3
    }

    fn reset(&mut self) {
        self.reg = [
            1937774191, 1226093241, 388252375, 3666478592,
            2842636476, 372324522, 3817729613, 2969243214
        ];
        self.chunk.clear();
        self.size = 0;
    }

    fn write(&mut self, data: &[u8]) {
        self.size += data.len();
        let mut offset = 0;
        while offset < data.len() {
            let space = 64 - self.chunk.len();
            let chunk_len = std::cmp::min(space, data.len() - offset);
            self.chunk.extend_from_slice(&data[offset..offset + chunk_len]);
            offset += chunk_len;
            if self.chunk.len() == 64 {
                self._compress(&self.chunk.clone());
                self.chunk.clear();
            }
        }
    }

    fn _fill(&mut self) {
        let bit_length = (8 * self.size) as u64;
        let padding_pos = self.chunk.len();
        self.chunk.push(0x80);
        
        let pos = (padding_pos + 1) % 64;
        let needed = if 64 - pos < 8 { (64 - pos) + 56 } else { 56 - pos };
        for _ in 0..needed {
            self.chunk.push(0);
        }
        for i in (0..8).rev() {
            self.chunk.push(((bit_length >> (i * 8)) & 0xFF) as u8);
        }
    }

    fn _compress(&mut self, data: &[u8]) {
        let mut w = [0u32; 132];
        for t in 0..16 {
            w[t] = ((data[4 * t] as u32) << 24)
                | ((data[4 * t + 1] as u32) << 16)
                | ((data[4 * t + 2] as u32) << 8)
                | (data[4 * t + 3] as u32);
        }
        for j in 16..68 {
            let a = w[j - 16] ^ w[j - 9] ^ left_rotate(w[j - 3], 15);
            let a = a ^ left_rotate(a, 15) ^ left_rotate(a, 23);
            w[j] = a ^ left_rotate(w[j - 13], 7) ^ w[j - 6];
        }
        for j in 0..64 {
            w[j + 68] = w[j] ^ w[j + 4];
        }

        let mut a = self.reg[0];
        let mut b = self.reg[1];
        let mut c = self.reg[2];
        let mut d = self.reg[3];
        let mut e = self.reg[4];
        let mut f = self.reg[5];
        let mut g = self.reg[6];
        let mut h = self.reg[7];

        for j in 0..64 {
            let ss1 = left_rotate(
                left_rotate(a, 12)
                    .wrapping_add(e)
                    .wrapping_add(left_rotate(get_t_j(j), j as u32)),
                7,
            );
            let ss2 = ss1 ^ left_rotate(a, 12);
            let tt1 = ff_j(j, a, b, c)
                .wrapping_add(d)
                .wrapping_add(ss2)
                .wrapping_add(w[j + 68]);
            let tt2 = gg_j(j, e, f, g)
                .wrapping_add(h)
                .wrapping_add(ss1)
                .wrapping_add(w[j]);

            d = c;
            c = left_rotate(b, 9);
            b = a;
            a = tt1;
            h = g;
            g = left_rotate(f, 19);
            f = e;
            e = tt2 ^ left_rotate(tt2, 9) ^ left_rotate(tt2, 17);
        }

        self.reg[0] ^= a;
        self.reg[1] ^= b;
        self.reg[2] ^= c;
        self.reg[3] ^= d;
        self.reg[4] ^= e;
        self.reg[5] ^= f;
        self.reg[6] ^= g;
        self.reg[7] ^= h;
    }

    fn sum(&mut self, data: Option<&[u8]>) -> Vec<u8> {
        if let Some(d) = data {
            self.reset();
            self.write(d);
        }
        self._fill();
        let chunk_data = self.chunk.clone();
        for f in (0..chunk_data.len()).step_by(64) {
            self._compress(&chunk_data[f..f + 64]);
        }

        let mut result = Vec::with_capacity(32);
        for f in 0..8 {
            let c = self.reg[f];
            result.push(((c >> 24) & 0xFF) as u8);
            result.push(((c >> 16) & 0xFF) as u8);
            result.push(((c >> 8) & 0xFF) as u8);
            result.push((c & 0xFF) as u8);
        }
        self.reset();
        result
    }
}

fn get_long_int(round_num: usize, long_str: &[u8]) -> u32 {
    let base = round_num * 3;
    let char1 = if base < long_str.len() { long_str[base] as u32 } else { 0 };
    let char2 = if base + 1 < long_str.len() { long_str[base + 1] as u32 } else { 0 };
    let char3 = if base + 2 < long_str.len() { long_str[base + 2] as u32 } else { 0 };
    (char1 << 16) | (char2 << 8) | char3
}

fn result_encrypt(long_str: &[u8], num: &str) -> String {
    let encoding_tables = [
        ("s0", "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/="),
        ("s1", "Dkdpgh4ZKsQB80/Mfvw36XI1R25+WUAlEi7NLboqYTOPuzmFjJnryx9HVGcaStCe="),
        ("s2", "Dkdpgh4ZKsQB80/Mfvw36XI1R25-WUAlEi7NLboqYTOPuzmFjJnryx9HVGcaStCe="),
        ("s3", "ckdp1h4ZKsUB80/Mfvw36XIgR25+WQAlEi7NLboqYTOPuzmFjJnryx9HVGDaStCe"),
        ("s4", "Dkdpgh2ZmsQB80/MfvV36XI1R45-WUAlEixNLwoqYTOPuzKFjJnry79HbGcaStCe"),
    ];
    let encoding_table = encoding_tables.iter()
        .find(|&&(n, _)| n == num)
        .map(|&(_, t)| t)
        .unwrap();

    let masks = [16515072u32, 258048u32, 4032u32, 63u32];
    let shifts = [18, 12, 6, 0];

    let mut result = String::new();
    let mut round_num = 0;
    let total_chars = ((long_str.len() as f64) / 3.0 * 4.0).ceil() as usize;

    for i in 0..total_chars {
        if i / 4 != round_num {
            round_num += 1;
        }
        let long_int = get_long_int(round_num, long_str);
        let index = i % 4;
        let char_index = ((long_int & masks[index]) >> shifts[index]) as usize;
        result.push(encoding_table.chars().nth(char_index).unwrap());
    }
    result
}

fn gener_random(random_num: i32, option: &[u8; 2]) -> Vec<u8> {
    let byte1 = (random_num & 255) as u8;
    let byte2 = ((random_num >> 8) & 255) as u8;
    vec![
        (byte1 & 170) | (option[0] & 85),
        (byte1 & 85) | (option[0] & 170),
        (byte2 & 170) | (option[1] & 85),
        (byte2 & 85) | (option[1] & 170),
    ]
}

fn generate_random_str() -> Vec<u8> {
    let random_values = [0.123456789f64, 0.987654321f64, 0.555555555f64];
    let mut random_bytes = Vec::new();
    random_bytes.extend(gener_random((random_values[0] * 10000.0) as i32, &[3, 45]));
    random_bytes.extend(gener_random((random_values[1] * 10000.0) as i32, &[1, 0]));
    random_bytes.extend(gener_random((random_values[2] * 10000.0) as i32, &[1, 5]));
    random_bytes
}

fn generate_rc4_bb_str(url_search_params: &str, user_agent: &str, window_env_str: &str) -> Vec<u8> {
    let suffix = "cus";
    let arguments = [0, 1, 14];
    let mut sm3 = SM3::new();
    
    let start_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let mut payload1 = url_search_params.to_string();
    payload1.push_str(suffix);
    let pass1 = sm3.sum(Some(payload1.as_bytes()));
    let url_search_params_list = sm3.sum(Some(&pass1));

    let pass2 = sm3.sum(Some(suffix.as_bytes()));
    let cus = sm3.sum(Some(&pass2));

    let ua_key = [0u8, 1u8, 14u8];
    let ua_rc4 = rc4_encrypt(user_agent.as_bytes(), &ua_key);
    let ua_b64 = result_encrypt(&ua_rc4, "s3");
    let ua = sm3.sum(Some(ua_b64.as_bytes()));

    let end_time = start_time + 100;
    let mut b = [0u8; 73];
    b[8] = 3;
    
    let split_to_bytes = |num: u64| -> [u8; 4] {
        [
            ((num >> 24) & 255) as u8,
            ((num >> 16) & 255) as u8,
            ((num >> 8) & 255) as u8,
            (num & 255) as u8,
        ]
    };

    let start_time_bytes = split_to_bytes(start_time);
    b[20] = start_time_bytes[0];
    b[21] = start_time_bytes[1];
    b[22] = start_time_bytes[2];
    b[23] = start_time_bytes[3];
    b[24] = ((start_time / 256 / 256 / 256 / 256) & 255) as u8;
    b[25] = ((start_time / 256 / 256 / 256 / 256 / 256) & 255) as u8;

    let arg0_bytes = split_to_bytes(arguments[0]);
    b[26] = arg0_bytes[0];
    b[27] = arg0_bytes[1];
    b[28] = arg0_bytes[2];
    b[29] = arg0_bytes[3];

    b[30] = ((arguments[1] / 256) & 255) as u8;
    b[31] = (arguments[1] % 256) as u8;

    let arg1_bytes = split_to_bytes(arguments[1]);
    b[32] = arg1_bytes[0];
    b[33] = arg1_bytes[1];

    let arg2_bytes = split_to_bytes(arguments[2]);
    b[34] = arg2_bytes[0];
    b[35] = arg2_bytes[1];
    b[36] = arg2_bytes[2];
    b[37] = arg2_bytes[3];

    b[38] = url_search_params_list[21];
    b[39] = url_search_params_list[22];
    b[40] = cus[21];
    b[41] = cus[22];
    b[42] = ua[23];
    b[43] = ua[24];

    let end_time_bytes = split_to_bytes(end_time);
    b[44] = end_time_bytes[0];
    b[45] = end_time_bytes[1];
    b[46] = end_time_bytes[2];
    b[47] = end_time_bytes[3];
    b[48] = b[8];
    b[49] = ((end_time / 256 / 256 / 256 / 256) & 255) as u8;
    b[50] = ((end_time / 256 / 256 / 256 / 256 / 256) & 255) as u8;

    let page_id = 110624u64;
    let page_id_bytes = split_to_bytes(page_id);
    b[52] = page_id_bytes[0];
    b[53] = page_id_bytes[1];
    b[54] = page_id_bytes[2];
    b[55] = page_id_bytes[3];

    let aid = 6383u64;
    b[57] = (aid & 255) as u8;
    b[58] = ((aid >> 8) & 255) as u8;
    b[59] = ((aid >> 16) & 255) as u8;
    b[60] = ((aid >> 24) & 255) as u8;

    let window_env_list: Vec<u8> = window_env_str.bytes().collect();
    b[64] = window_env_list.len() as u8;
    b[65] = (window_env_list.len() & 255) as u8;
    b[66] = ((window_env_list.len() >> 8) & 255) as u8;

    b[72] = b[18] ^ b[20] ^ b[26] ^ b[30] ^ b[38] ^ b[40] ^ b[42] ^ b[21] ^ b[27] ^ b[31] ^
            b[35] ^ b[39] ^ b[41] ^ b[43] ^ b[22] ^ b[28] ^ b[32] ^ b[36] ^ b[23] ^ b[29] ^
            b[33] ^ b[37] ^ b[44] ^ b[45] ^ b[46] ^ b[47] ^ b[48] ^ b[49] ^ b[50] ^ b[24] ^
            b[25] ^ b[52] ^ b[53] ^ b[54] ^ b[55] ^ b[57] ^ b[58] ^ b[59] ^ b[60] ^ b[65] ^
            b[66] ^ b[70] ^ b[71];

    let mut bb = vec![
        b[18], b[20], b[52], b[26], b[30], b[34], b[58], b[38], b[40], b[53], b[42], b[21],
        b[27], b[54], b[55], b[31], b[35], b[57], b[39], b[41], b[43], b[22], b[28], b[32],
        b[60], b[36], b[23], b[29], b[33], b[37], b[44], b[45], b[59], b[46], b[47], b[48],
        b[49], b[50], b[24], b[25], b[65], b[66], b[70], b[71]
    ];
    bb.extend(window_env_list);
    bb.push(b[72]);

    rc4_encrypt(&bb, &[121])
}

pub fn ab_sign(url_search_params: &str, user_agent: &str) -> String {
    let window_env_str = "1920|1080|1920|1040|0|30|0|0|1872|92|1920|1040|1857|92|1|24|Win32";
    let mut payload = generate_random_str();
    payload.extend(generate_rc4_bb_str(url_search_params, user_agent, window_env_str));
    format!("{}=", result_encrypt(&payload, "s4"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ab_sign() {
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
        let query = "aid=6383&app_name=douyin_web&live_id=1&device_platform=web&language=zh-CN&browser_language=zh-CN&browser_platform=Win32&browser_name=Chrome&browser_version=116.0.0.0&web_rid=335354047186&msToken=";
        let sign = ab_sign(query, ua);
        assert!(!sign.is_empty());
        assert!(sign.ends_with('='));
        println!("Generated a_bogus: {}", sign);
    }
}
