//! Guest image entry point. The host machine role does not invoke it directly.

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    use std::{fs::File, io::Read, process::ExitCode};

    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--reseed-small-boot")
    {
        return match reseed_small_boot() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => {
                eprintln!("Small plugin VM kernel reseed failed");
                ExitCode::FAILURE
            }
        };
    }
    let mut token = [0; 32];
    if File::open("/dev/vdc")
        .and_then(|mut file| file.read_exact(&mut token))
        .is_err()
    {
        eprintln!("Guest control capability is unavailable");
        return ExitCode::FAILURE;
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("guest runtime");
    let result = runtime.block_on(calternal_machine::sandbox::microvm::run_agent(
        "/run/agent/control.sock".into(),
        "/data/sandboxes".into(),
        "/opt/standard:O".into(),
        token,
    ));
    if result.is_err() {
        eprintln!("Guest sandbox agent failed");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(target_os = "linux")]
fn reseed_small_boot() -> std::io::Result<()> {
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Seek, SeekFrom, Write};

    let mut control = File::open("/dev/vdc")?;
    control.seek(SeekFrom::Start(32))?;
    let mut seed = [0; 32];
    control.read_exact(&mut seed)?;
    let mut pool = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/urandom")?;
    pool.write_all(&seed)?;
    // SAFETY: Linux RNDRESEEDCRNG is a no-argument ioctl on the random device.
    unsafe {
        rustix::ioctl::ioctl(&pool, rustix::ioctl::NoArg::<0x5207>::new())?;
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    std::process::ExitCode::FAILURE
}
