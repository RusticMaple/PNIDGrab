use anyhow::{anyhow, Context, Result};
use libc::{
    c_void, mach_task_self, mach_vm_region, mach_vm_read, task_for_pid, vm_deallocate,
    vm_region_basic_info_64, KERN_SUCCESS, VM_PROT_READ, VM_REGION_BASIC_INFO_64,
};
use libproc::libproc::proc_pid::listpids;
use libproc::libproc::proc_pid::ProcType;
use std::mem;

use super::ProcessMemory;

const TARGET_NAMES: &[&str] = &["cemu", "cemu_release"];
const PATTERN: [u8; 3] = [0x02, 0xD4, 0xE7];
const MIN_REGION_SIZE: u64 = 1308622848;
const VM_REGION_BASIC_INFO_COUNT_64: u32 = 10;

pub struct MacProcessMemory {
    task: libc::mach_port_t,
    base_address: u64,
}

impl ProcessMemory for MacProcessMemory {
    fn new(pid: i32) -> Result<Self> {
        unsafe {
            let mut task: libc::mach_port_t = 0;
            let result = task_for_pid(mach_task_self(), pid, &mut task);
            
            if result != KERN_SUCCESS {
                return Err(anyhow!("task_for_pid failed with code {}", result));
            }

            let base_address = find_cemu_base(task)?;
            Ok(Self { task, base_address })
        }
    }

    fn read_bytes(&self, address: u32, length: usize) -> Result<Vec<u8>> {
        unsafe {
            let mut data: *mut c_void = std::ptr::null_mut();
            let mut data_count: libc::mach_vm_size_t = 0;

            let result = mach_vm_read(
                self.task,
                self.base_address + address as u64,
                length as libc::mach_vm_size_t,
                &mut data,
                &mut data_count,
            );

            if result != KERN_SUCCESS {
                return Err(anyhow!("mach_vm_read failed with code {}", result));
            }

            let buffer = std::slice::from_raw_parts(data as *const u8, data_count as usize).to_vec();
            vm_deallocate(mach_task_self(), data as libc::vm_address_t, data_count);

            Ok(buffer)
        }
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

unsafe fn find_cemu_base(task: libc::mach_port_t) -> Result<u64> {
    let mut address: u64 = 0;
    
    loop {
        let mut size: u64 = 0;
        let mut count = VM_REGION_BASIC_INFO_COUNT_64;
        let mut info: vm_region_basic_info_64 = mem::zeroed();
        let mut object_name: u64 = 0;

        let result = mach_vm_region(
            task,
            &mut address,
            &mut size,
            VM_REGION_BASIC_INFO_64 as i32,
            &mut info as *mut _ as *mut _,
            &mut count,
            &mut object_name,
        );

        if result != KERN_SUCCESS {
            break;
        }

        let readable = (info.protection & VM_PROT_READ) != 0;
        
        if readable && size >= MIN_REGION_SIZE {
            let mut data: *mut c_void = std::ptr::null_mut();
            let mut data_count: libc::mach_vm_size_t = 0;

            let read_address = address + 0xE000000;
            let read_result = mach_vm_read(
                task,
                read_address,
                20,
                &mut data,
                &mut data_count,
            );

            if read_result == KERN_SUCCESS {
                let buffer = std::slice::from_raw_parts(data as *const u8, data_count as usize);
                if buffer.windows(3).any(|w| w == PATTERN) {
                    vm_deallocate(mach_task_self(), data as libc::vm_address_t, data_count);
                    return Ok(address);
                }
                vm_deallocate(mach_task_self(), data as libc::vm_address_t, data_count);
            }
        }

        address += size;
    }

    Err(anyhow!("Cemu base not found"))
}

pub fn find_cemu_process() -> Result<i32> {
    // Get all process IDs
    let pids = match listpids(ProcType::ProcAllPIDS) {
        Ok(pids) => pids,
        Err(_) => return Err(anyhow!("Failed to get process list")),
    };

    // Check each process
    for &pid in &pids {
        if pid <= 0 {
            continue;
        }

        // Get process name
        if let Ok(info) = libproc::libproc::proc_pid::pidinfo::<libproc::libproc::proc_pid::ProcTaskInfo>(pid, 0) {
            let name = std::ffi::CStr::from_bytes_with_nul(&info.pbsd.pbi_name)
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default();

            if TARGET_NAMES.contains(&name) {
                return Ok(pid);
            }
        }
    }

    Err(anyhow!("Cemu process not found"))
}
