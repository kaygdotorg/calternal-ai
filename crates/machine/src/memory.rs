//! Machine-local memory telemetry for placement heartbeats.

/// Physical and available bytes. Read the Linux kernel snapshot once per sync.
#[cfg(target_os = "linux")]
pub(super) fn sample() -> (u64, Option<u64>) {
    let Ok(info) = std::fs::read_to_string("/proc/meminfo") else {
        return (0, None);
    };
    let bytes = |key| {
        let mut fields = info.lines().find_map(|line| line.strip_prefix(key))?.split_whitespace();
        let kib = fields.next()?.parse::<u64>().ok()?;
        (fields.next()? == "kB").then(|| kib.checked_mul(1024)).flatten()
    };
    (bytes("MemTotal:").unwrap_or(0), bytes("MemAvailable:"))
}

/// Available bytes from the kernel's free pages, excluding inactive pages.
#[cfg(target_os = "macos")]
pub(super) fn sample() -> (u64, Option<u64>) {
    fn available() -> Option<u64> {
        let output = std::process::Command::new("/usr/bin/vm_stat").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = std::str::from_utf8(&output.stdout).ok()?;
        let (header, counts) = text.split_once('\n')?;
        let (_, size) = header.split_once("page size of ")?;
        let size = size.split_whitespace().next()?.parse::<u64>().ok()?;
        let pages = counts.lines().find_map(|line| line.strip_prefix("Pages free:"))?;
        pages.trim().trim_end_matches('.').parse::<u64>().ok()?.checked_mul(size)
    }
    (0, available())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(super) fn sample() -> (u64, Option<u64>) {
    (0, None)
}
