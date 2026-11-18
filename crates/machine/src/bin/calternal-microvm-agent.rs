//! Guest image entry point. The host machine role does not invoke it directly.

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    use std::{fs::File, io::Read, process::ExitCode};

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
        "localhost/calternal-standard:current".into(),
        token,
    ));
    if result.is_err() {
        eprintln!("Guest sandbox agent failed");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    std::process::ExitCode::FAILURE
}
