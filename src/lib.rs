mod log;
mod interfaces;
mod interface_reg;

use std::ptr::*;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::System::Memory::*;
use windows::Win32::System::ProcessStatus::*;
use windows::Win32::System::SystemServices::*;
use windows::Win32::System::Threading::*;

use log::*;
use interface_reg::*;

#[unsafe(no_mangle)]
pub extern "system" fn DllMain(
    _hinst: HINSTANCE,
    reason: u32,
    _: *mut core::ffi::c_void,
) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        run_patch();
        spawn_logger();
        reset_replay().unwrap();
        let _ = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open("log.txt");
    }

    TRUE
}

/// Patching
unsafe fn patch_call(
    call_addr: *mut u8,
    target_addr: usize,
)
{
    unsafe {
        let next_instr = call_addr.add(5) as usize;

        let rel =
            (target_addr as isize - next_instr as isize) as i32;

        let mut old = PAGE_PROTECTION_FLAGS(0);

        let _ = VirtualProtect(
            call_addr as _,
            5,
            PAGE_EXECUTE_READWRITE,
            &mut old,
        );

        *call_addr = 0xE8;

        std::ptr::write_unaligned(
            call_addr.add(1) as *mut i32,
            rel,
        );

        let _ = VirtualProtect(
            call_addr as _,
            5,
            old,
            &mut old,
        );
    }
}

fn run_patch()
{
    unsafe {
        let exe = GetModuleHandleA(None).unwrap();
        let start = exe.0 as *const u8;

        let mut info = MODULEINFO {
            lpBaseOfDll: null_mut(),
            SizeOfImage: 0,
            EntryPoint: null_mut()
        };

        let _ = GetModuleInformation(
            GetCurrentProcess(),
            exe,
            &mut info,
            std::mem::size_of::<MODULEINFO>() as u32,
        );
        let end = start.wrapping_add(info.SizeOfImage.try_into().unwrap());

        let mut p = start;

        while p < end {
            if *p == 0xE8 {
                let disp =
                    std::ptr::read_unaligned(
                        p.add(1) as *const i32
                    );

                let address = p.add(5) as isize;

                let destination = address + disp as isize;

                for repl in replacements() {
                    if repl.rva != 0x0 {
                        if destination as usize == repl.rva {
                            patch_call(
                                p as *mut u8,
                                repl.replacement.unwrap()(),
                            );
                        }
                    }
                }
            }

            p = p.add(1);
        }
    }
}

