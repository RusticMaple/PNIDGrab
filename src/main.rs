// Credits where credits due: 
// - c8ff for finding the Cemu base adress
// - javiig8 for finding the adresses for PID and Name
// - ReXiSp for finding the address for the Session ID

use anyhow::{anyhow, Context, Result};
use libc::{c_void, iovec, process_vm_readv};
use reqwest::blocking::Client;
use roxmltree::Document;
use std::{
    fs,
    io::{BufRead, BufReader},
    time::SystemTime,
};

const TARGET_NAMES: &[&str] = &["cemu", "xapfish", ".cemu-wrapped"];
const PATTERN: [u8; 3] = [0x02, 0xD4, 0xE7];
const MIN_REGION_SIZE: u64 = 1308622848;

struct MemoryRegion {
    start: u64,
    end: u64,
    permissions: String,
}

struct ProcessMemory {
    pid: i32,
    base_address: u64,
}

impl ProcessMemory {
    fn read_bytes(&self, address: u32, length: usize) -> Result<Vec<u8>> {
        let target = self.base_address + address as u64;
        let mut buffer = vec![0u8; length];

        let local_iov = iovec {
            iov_base: buffer.as_mut_ptr() as *mut c_void,
            iov_len: length,
        };

        let remote_iov = iovec {
            iov_base: target as *mut c_void,
            iov_len: length,
        };

        let result = unsafe {
            process_vm_readv(
                self.pid,
                &local_iov as *const iovec,
                1,
                &remote_iov as *const iovec,
                1,
                0,
            )
        };

        if result == -1 {
            return Err(std::io::Error::last_os_error()).context("process_vm_readv failed");
        }

        let nread = result as usize;
        if nread < length {
            buffer.truncate(nread);
        }

        Ok(buffer)
    }

    fn read_u32(&self, address: u32) -> Result<u32> {
        let bytes = self.read_bytes(address, 4)?;
        let arr: [u8; 4] = bytes
            .try_into()
            .map_err(|_| anyhow!("Failed to convert 4-byte vec to array"))?;
        Ok(u32::from_be_bytes(arr))
    }

    fn read_u8(&self, address: u32) -> Result<u8> {
        let bytes = self.read_bytes(address, 1)?;
        Ok(bytes[0])
    }
}

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

fn find_cemu_process() -> Result<i32> {
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let filename = entry.file_name();
        let pid_str = match filename.to_str() {
            Some(s) => s,
            None => continue,
        };

        let pid = match pid_str.parse::<i32>() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let comm_path = format!("/proc/{}/comm", pid);
        let comm = match fs::read_to_string(&comm_path) {
            Ok(c) => c.trim().to_string(),
            Err(_) => continue,
        };

        if TARGET_NAMES.contains(&comm.as_str()) {
            return Ok(pid);
        }
    }
    Err(anyhow!("Cemu process not found"))
}

fn parse_maps(pid: i32) -> Result<Vec<MemoryRegion>> {
    let path = format!("/proc/{}/maps", pid);
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut regions = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let addrs: Vec<&str> = parts[0].split('-').collect();
        if addrs.len() != 2 {
            continue;
        }

        let start = u64::from_str_radix(addrs[0], 16)?;
        let end = u64::from_str_radix(addrs[1], 16)?;
        let permissions = parts[1].to_string();

        regions.push(MemoryRegion {
            start,
            end,
            permissions,
        });
    }
    Ok(regions)
}

fn find_suitable_region(regions: &[MemoryRegion]) -> Result<&MemoryRegion> {
    regions
        .iter()
        .find(|r| {
            r.permissions.contains('r')
                && (r.end - r.start) >= MIN_REGION_SIZE
        })
        .ok_or_else(|| anyhow!("No suitable memory region found"))
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
    println!("PNIDGrab 1.0.1 by jerrysm64 (Jerry)");

    let pid = find_cemu_process()?;
    let regions = parse_maps(pid)?;
    let region = find_suitable_region(&regions)?;
    let base_address = region.start + 0xE000000 - 0x10000000;

    let proc_mem = ProcessMemory { pid, base_address };

    let check_bytes = proc_mem.read_bytes(0x10000000, 20)?;
    if !check_bytes.windows(3).any(|w| w == PATTERN) {
        return Err(anyhow!("Memory pattern not found"));
    }

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

    let now = SystemTime::now();
    let datetime: chrono::DateTime<chrono::Local> = now.into();
    println!("\nFetched at: {}", datetime.format("%Y-%m-%d %H:%M:%S"));

    Ok(())
}
