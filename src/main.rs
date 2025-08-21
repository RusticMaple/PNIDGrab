// Credits where credits due:
// - c8ff for finding the Cemu base address
// - javiig8 for finding the addresses for PID and Name
// - ReXiSp for finding the address for the Session ID
// - CrafterPika for helping me with the macOS and Windows implementations
// - RusticMaple for the idea how to split platforms without anything clashing

use anyhow::Result;
use chrono::Local;
use platform::{ProcessMemory, find_cemu_process};
use reqwest::blocking::Client;
use roxmltree::Document;
use std::time::SystemTime;

mod platform;

fn get_pnid(pid: i32) -> String {
    let client = match Client::builder()
        .user_agent("Mozilla/5.0")
        .build()
    {
        Ok(c) => c,
        Err(_) => return "0".to_string(),
    };

    // API key src: https://github.com/kinnay/NintendoClients/wiki/Account-Server
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
        return "0".to_string();
    }

    let body = match response.text() {
        Ok(b) => b,
        Err(_) => return "0".to_string(),
    };

    let doc = match Document::parse(&body) {
        Ok(d) => d,
        Err(_) => return "0".to_string(),
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

fn main() -> Result<()> {
    println!("PNIDGrab 2.0.0 by jerrysm64 (Jerry)");

    let pid = find_cemu_process()?;

    #[cfg(target_os = "linux")]
    let proc_mem = platform::LinuxProcessMemory::new(pid)?;

    #[cfg(target_os = "windows")]
    let proc_mem = platform::WindowsProcessMemory::new(pid)?;

    #[cfg(target_os = "macos")]
    let proc_mem = platform::MacProcessMemory::new(pid)?;

    println!("Player X: PID (Hex)| PID (Dec)  | PNID             | Name");
    println!("---------------------------------------------------------------");

    let ptr1 = proc_mem.read_u32(0x101DD330)?;
    let ptr2 = proc_mem.read_u32(ptr1 + 0x10)?;

    for i in 0..8 {
        let player_ptr = proc_mem.read_u32(ptr2 + (i * 4))?;
        if player_ptr == 0 {
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
        let nnid_str = format!("{:<16}", nnid);

        println!(
            "Player {}: {} | {:<10} | {} | {}",
            i, pid_hex, pid_raw, nnid_str, name
        );
    }

    let ptr = proc_mem.read_u32(0x101E8980)?;
    if ptr != 0 {
        let index = proc_mem.read_u8(ptr + 0xBD)?;
        let session_id = proc_mem.read_u32(ptr + index as u32 + 0xCC)?;
        println!("\nSession ID: {:08X} (Dec: {})", session_id, session_id);
    } else {
        println!("\nSession ID: None");
    }

    let _now = SystemTime::now();
    let datetime = Local::now();
    println!("\nFetched at: {}", datetime.format("%Y-%m-%d %H:%M:%S"));

    Ok(())
}
