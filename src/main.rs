use anyhow::Result;
use chrono::{DateTime, Local};
use roxmltree::Document;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

mod platform;
mod gui;

use platform::{find_cemu_process};
use platform::ProcessMemory;

#[cfg(target_os = "linux")]
use platform::LinuxProcessMemory as PlatformProcessMemory;

#[cfg(target_os = "windows")]
use platform::WindowsProcessMemory as PlatformProcessMemory;

#[cfg(target_os = "macos")]
use platform::MacProcessMemory as PlatformProcessMemory;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerRecord {
    pub index: u8,
    pub pid_hex: String,
    pub pid_dec: u32,
    pub pnid: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchResult {
    pub players: Vec<PlayerRecord>,
    pub session_id: Option<u32>,
    pub fetched_at: DateTime<Local>,
}

fn get_pnid(pid: i32) -> String {
    let client = match Client::builder().user_agent("Mozilla/5.0").build() {
        Ok(c) => c,
        Err(_) => return "0".to_string(),
    };

    let url = format!("http://account.pretendo.cc/v1/api/miis?pids={}", pid);
    let response = match client
        .get(&url)
        .header("X-Nintendo-Client-ID", "a2efa818a34fa16b8afbc8a74eba3eda")
        .header("X-Nintendo-Client-Secret", "c91cdb5658bd4954ade78533a339cf9a")
        .send()
    {
        Ok(r) => r,
        Err(_) => return "0".to_string(),
    };

    if !response.status().is_success() {
        return "0".to_string()
    }

    let body = match response.text() {
        Ok(b) => b,
        Err(_) => return "0".to_string()
    };

    let doc = match Document::parse(&body) {
        Ok(d) => d,
        Err(_) => return "0".to_string()
    };

    doc.descendants()
        .find(|n| n.tag_name().name() == "user_id")
        .and_then(|n| n.text())
        .unwrap_or("0")
        .to_string()
}

fn decode_name(bytes: &[u8]) -> String {
    let mut name_chars = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let code = u16::from_be_bytes([chunk[0], chunk[1]]);
        if code == 0 {
            break;
        }
        name_chars.push(code);
    }

    String::from_utf16_lossy(&name_chars)
        .trim()
        .replace(['\n', '\r'], "")
}

pub fn fetch_all() -> Result<FetchResult> {
    let pid = find_cemu_process()?;

    let proc_mem = PlatformProcessMemory::new(pid)?;

    let mut players: Vec<PlayerRecord> = Vec::new();

    let ptr1 = proc_mem.read_u32(0x101DD330)?;
    let ptr2 = proc_mem.read_u32(ptr1 + 0x10)?;

    for i in 0..8 {
        let player_ptr = proc_mem.read_u32(ptr2 + (i * 4))?;
        if player_ptr == 0 {
            players.push(PlayerRecord {
                index: i as u8,
                pid_hex: "00000000".to_string(),
                pid_dec: 0,
                pnid: "0".to_string(),
                name: "????????".to_string(),
            });
            continue;
        }

        let name_bytes = proc_mem.read_bytes(player_ptr + 0x6, 32)?;
        let name = decode_name(&name_bytes);
        let pid_raw = proc_mem.read_u32(player_ptr + 0xD0)?;
        let pid_bytes = pid_raw.to_le_bytes();
        let pid_hex = format!(
            "{:02X}{:02X}{:02X}{:02X}",
            pid_bytes[0], pid_bytes[1], pid_bytes[2], pid_bytes[3]
        );
        let nnid = get_pnid(pid_raw as i32);

        players.push(PlayerRecord {
            index: i as u8,
            pid_hex,
            pid_dec: pid_raw,
            pnid: nnid,
            name,
        });
    }

    let ptr = proc_mem.read_u32(0x101E8980)?;
    let session_id = if ptr != 0 {
        let index = proc_mem.read_u8(ptr + 0xBD)?;
        let session_id = proc_mem.read_u32(ptr + index as u32 + 0xCC)?;
        Some(session_id)
    } else {
        None
    };

    let datetime = Local::now();

    Ok(FetchResult {
        players,
        session_id,
        fetched_at: datetime,
    })
}

fn main() -> Result<()> {
    gui::run_app()
}
