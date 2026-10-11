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
    use super::memory_cleaners_in;

    #[test]
    fn a_memory_cleaner_is_spotted_in_the_list_of_programs() {
        let programs = "\"explorer.exe\",\"1234\",\"Console\",\"1\",\"90,000 K\"\r\n\"MemReduct.exe\",\"4321\",\"Console\",\"1\",\"8,000 K\"\r\n";
        assert_eq!(memory_cleaners_in(programs), vec!["Mem Reduct"]);
        assert!(memory_cleaners_in("\"notmemreduct.exe\",\"1\"\r\n\"explorer.exe\",\"2\"\r\n").is_empty());
    }
}
