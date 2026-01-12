// src/hardware/protocol.rs
use std::error::Error;
use base64::Engine; // Add this import

/// Command types that can be sent to the hardware wallet
#[derive(Debug, Clone)]
pub enum Command {
    GetPubkey,
    SignMessage(Vec<u8>),
}

/// Response types from the hardware wallet
#[derive(Debug, Clone)]
pub enum Response {
    Pubkey(String),
    Signature(Vec<u8>),
    Error(String),
}

/// Convert the protocol to match ESP32 expectations
pub fn format_esp32_command(cmd: &Command) -> Vec<u8> {
    match cmd {
        Command::GetPubkey => b"GET_PUBKEY\n".to_vec(),
        Command::SignMessage(data) => {
            let mut formatted = b"SIGN:".to_vec();
            // Use the standard base64 engine
            let encoded = base64::engine::general_purpose::STANDARD.encode(data);
            formatted.extend_from_slice(encoded.as_bytes());
            formatted.push(b'\n');
            formatted
        }
    }
}

/// Parse ESP32 response format
pub fn parse_esp32_response(data: &[u8]) -> Result<Response, Box<dyn Error>> {
    let is_base58 = |b: u8| matches!(b,
        b'1'..=b'9' |
        b'A'..=b'H' |
        b'J'..=b'N' |
        b'P'..=b'Z' |
        b'a'..=b'k' |
        b'm'..=b'z'
    );
    let is_base64 = |b: u8| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=';

    let find_bytes = |haystack: &[u8], needle: &[u8]| -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    };

    if let Some(pos) = find_bytes(data, b"PUBKEY:") {
        let mut idx = pos + b"PUBKEY:".len();
        let mut out = Vec::new();
        while idx < data.len() {
            if idx + b"READY".len() <= data.len() && &data[idx..idx + b"READY".len()] == b"READY" {
                break;
            }
            let b = data[idx];
            if b == b'\n' || b == b'\r' {
                break;
            }
            if b == 0x00 {
                idx += 1;
                continue;
            }
            if is_base58(b) {
                out.push(b);
                idx += 1;
                continue;
            }
            break;
        }
        let pubkey_clean = String::from_utf8_lossy(&out).to_string();
        if pubkey_clean.len() < 32 {
            log::warn!("Short pubkey parsed ({} chars): {}", pubkey_clean.len(), pubkey_clean);
        }
        Ok(Response::Pubkey(pubkey_clean))
    } else if let Some(pos) = find_bytes(data, b"SIGNATURE:") {
        let mut idx = pos + b"SIGNATURE:".len();
        let mut out = Vec::new();
        while idx < data.len() {
            if idx + b"READY".len() <= data.len() && &data[idx..idx + b"READY".len()] == b"READY" {
                break;
            }
            let b = data[idx];
            if b == b'\n' || b == b'\r' {
                break;
            }
            if b == 0x00 {
                idx += 1;
                continue;
            }
            if is_base64(b) {
                out.push(b);
                idx += 1;
                continue;
            }
            break;
        }
        let sig_clean = String::from_utf8_lossy(&out).to_string();
        let mut sig_clean = sig_clean;
        let rem = sig_clean.len() % 4;
        if rem != 0 {
            sig_clean.extend(std::iter::repeat('=').take(4 - rem));
        }
        let sig_bytes = base64::engine::general_purpose::STANDARD.decode(sig_clean)?;
        Ok(Response::Signature(sig_bytes))
    } else if let Some(pos) = find_bytes(data, b"ERROR:") {
        let mut idx = pos + b"ERROR:".len();
        let mut out = Vec::new();
        while idx < data.len() && !data[idx].is_ascii_whitespace() {
            out.push(data[idx]);
            idx += 1;
        }
        let err = String::from_utf8_lossy(&out).to_string();
        Ok(Response::Error(err))
    } else {
        let response_str = String::from_utf8_lossy(data);
        Err(format!("Unknown response format: {}", response_str).into())
    }
}
