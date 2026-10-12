//! Things about the Windows PC itself that a running Mac depends on.

use std::process::{Command, Stdio};

/// Programs that "free" memory by pushing other programs' memory out to disk, by the name of
/// their program file and the name to tell the user. They do this to a Mac's memory too, and
/// the Mac then crawls for a long time: one stalled a macOS installation here for an hour.
const MEMORY_CLEANERS: [(&str, &str); 6] = [
    ("memreduct.exe", "Mem Reduct"),
    ("wisememoryoptimzer.exe", "Wise Memory Optimizer"),
    ("wisememoryoptimizer.exe", "Wise Memory Optimizer"),
    ("cleanmem.exe", "CleanMem"),
    ("mem optimizer.exe", "Memory Optimizer"),
    ("ramrush.exe", "RAMRush"),
];

/// The name the processor's maker gave itself: "GenuineIntel", "AuthenticAMD" and so on.
pub fn cpu_vendor() -> String {
    let leaf = cpuid(0);
    registers_as_text(&[leaf.1, leaf.3, leaf.2])
}

/// The processor's own description of itself, such as "AMD Ryzen 7 5800X 8-Core Processor".
pub fn cpu_name() -> String {
    if cpuid(0x8000_0000).0 < 0x8000_0004 {
        return "unnamed processor".to_string();
    }
    let registers: Vec<u32> = (0x8000_0002..=0x8000_0004u32).flat_map(|leaf| <[u32; 4]>::from(cpuid(leaf))).collect();
    registers_as_text(&registers).trim().to_string()
}

/// The accelerator QEMU is started with on a processor from `vendor`.
///
/// On an Intel processor it is plain `whpx`. A Mac asks the processor for registers only
/// Intel processors have; on any other processor Windows' hypervisor does not know them, and
/// QEMU by default answers such a request with a fault, which kills macOS before it has put
/// anything on the screen. `ignore-unknown-msr` makes QEMU answer with zero instead, which is
/// what Linux's `ignore_msrs` does for the same Macs under KVM.
///
/// `intercept-msr-gp` cures nothing, but it makes QEMU write down every register request the
/// hypervisor itself refused. Those go to QEMU's log, which is in the diagnostic report, so a
/// Mac that still does not start says exactly what stopped it.
pub fn default_accelerator(vendor: &str) -> &'static str {
    if is_intel(vendor) { "whpx" } else { "whpx,ignore-unknown-msr=on,intercept-msr-gp=on" }
}

pub fn is_intel(vendor: &str) -> bool {
    vendor == "GenuineIntel"
}

/// The name of the boot folder for a processor from `vendor`, beside the program. The one
/// for other processors differs in its configuration only; see `orchord-efi`.
pub fn boot_folder_name(vendor: &str) -> &'static str {
    if is_intel(vendor) { "opencore" } else { "opencore-amd" }
}

/// The processor model QEMU is asked for, given the one a macOS version wants and who made
/// the PC's processor.
///
/// QEMU's models describe Intel processors. Three things the newer ones promise are missing
/// from many processors of other makers, and a Mac that believes the promise uses them and
/// dies: process-context identifiers (`pcid`, `invpcid`) and the timer that fires at a given
/// time-stamp count (`tsc-deadline`). macOS runs without all three, so on a processor that is
/// not Intel's they are not promised. An old model that never had them is left as it is.
pub fn guest_cpu(model: &str, vendor: &str) -> String {
    if vendor == "GenuineIntel" { model.to_string() } else { format!("{model},-pcid,-invpcid,-tsc-deadline") }
}

/// The registers EAX, EBX, ECX and EDX of one CPUID leaf.
fn cpuid(leaf: u32) -> (u32, u32, u32, u32) {
    #[cfg(target_arch = "x86_64")]
    {
        // CPUID exists on every 64-bit processor, so the call cannot go wrong.
        #[allow(unused_unsafe)]
        let found = unsafe { std::arch::x86_64::__cpuid(leaf) };
        (found.eax, found.ebx, found.ecx, found.edx)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = leaf;
        (0, 0, 0, 0)
    }
}

fn registers_as_text(registers: &[u32]) -> String {
    let bytes: Vec<u8> = registers.iter().flat_map(|register| register.to_le_bytes()).filter(|byte| *byte != 0).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The memory cleaners running on this PC right now, by the names users know them by.
pub fn memory_cleaners_running() -> Vec<&'static str> {
    memory_cleaners_in(&running_programs())
}

fn memory_cleaners_in(programs: &str) -> Vec<&'static str> {
    let programs = programs.to_lowercase();
    let mut found: Vec<&'static str> = MEMORY_CLEANERS.iter().filter(|(file, _)| programs.contains(&format!("\"{file}\""))).map(|(_, name)| *name).collect();
    found.dedup();
    found
}

/// The output of `tasklist` in its comma-separated form: one line per program, the first
/// field being the quoted name of its file.
fn running_programs() -> String {
    let mut command = Command::new("tasklist.exe");
    command.args(["/fo", "csv", "/nh"]).stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.output().map(|output| String::from_utf8_lossy(&output.stdout).into_owned()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{cpu_name, cpu_vendor, default_accelerator, guest_cpu, memory_cleaners_in};

    #[test]
    fn a_processor_that_is_not_intels_is_promised_less() {
        assert_eq!(guest_cpu("Skylake-Client,-hle,-rtm", "GenuineIntel"), "Skylake-Client,-hle,-rtm");
        assert_eq!(guest_cpu("Penryn", "AuthenticAMD"), "Penryn,-pcid,-invpcid,-tsc-deadline");
    }

    #[test]
    fn the_processor_names_itself_and_its_maker() {
        assert_eq!(cpu_vendor().len(), 12, "a maker's name is twelve characters: {}", cpu_vendor());
        assert!(!cpu_name().is_empty());
    }

    #[test]
    fn only_an_intel_processor_gets_the_plain_accelerator() {
        assert_eq!(default_accelerator("GenuineIntel"), "whpx");
        assert_eq!(default_accelerator("AuthenticAMD"), "whpx,ignore-unknown-msr=on,intercept-msr-gp=on");
        assert_eq!((super::boot_folder_name("GenuineIntel"), super::boot_folder_name("AuthenticAMD")), ("opencore", "opencore-amd"));
    }

    #[test]
    fn a_memory_cleaner_is_spotted_in_the_list_of_programs() {
        let programs = "\"explorer.exe\",\"1234\",\"Console\",\"1\",\"90,000 K\"\r\n\"MemReduct.exe\",\"4321\",\"Console\",\"1\",\"8,000 K\"\r\n";
        assert_eq!(memory_cleaners_in(programs), vec!["Mem Reduct"]);
        assert!(memory_cleaners_in("\"notmemreduct.exe\",\"1\"\r\n\"explorer.exe\",\"2\"\r\n").is_empty());
    }
}
