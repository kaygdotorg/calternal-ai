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
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--reseed-small-serve")
    {
        return match serve_small_reseed() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
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
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let mut control = File::open("/dev/vdc")?;
    control.seek(SeekFrom::Start(32))?;
    let mut seed = [0; 32];
    control.read_exact(&mut seed)?;
    reseed_kernel(&seed)
}

#[cfg(target_os = "linux")]
fn serve_small_reseed() -> std::io::Result<()> {
    use std::io::{ErrorKind, Read, Write};
    use std::os::unix::{
        fs::{PermissionsExt, chown},
        net::UnixListener,
    };
    use std::time::Duration;

    let path = "/run/calternal-reseed.sock";
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    chown(path, Some(1000), Some(1000))?;
    // PID 1 waits for this marker before starting the control agent.
    std::fs::write("/run/agent/small-ready", [])?;
    loop {
        let mut connection = match listener.accept() {
            Ok((connection, _)) => connection,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        let deadline = Some(Duration::from_secs(1));
        connection.set_read_timeout(deadline)?;
        connection.set_write_timeout(deadline)?;
        let mut seed = [0; 32];
        if connection.read_exact(&mut seed).is_ok() && reseed_kernel(&seed).is_ok() {
            seed.fill(0);
            let _ = connection.write_all(b"1");
        }
    }
}

#[cfg(target_os = "linux")]
fn reseed_kernel(seed: &[u8; 32]) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;
    let mut pool = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/urandom")?;
    pool.write_all(seed)?;
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
