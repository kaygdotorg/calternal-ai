//! Shell-free liveness probe for the runtime image.
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

fn main() -> io::Result<()> {
    let timeout = Duration::from_secs(2);
    let mut connection = TcpStream::connect_timeout(&([127, 0, 0, 1], 8080).into(), timeout)?;
    connection.set_read_timeout(Some(timeout))?;
    connection.set_write_timeout(Some(timeout))?;
    connection.write_all(
        b"GET /api/v1/auth/status HTTP/1.1\r\nHost: 127.0.0.1:8080\r\nConnection: close\r\n\r\n",
    )?;
    let mut response = String::new();
    BufReader::new(connection).read_line(&mut response)?;
    if response.starts_with("HTTP/1.1 200 ") {
        Ok(())
    } else {
        Err(io::Error::other("Calternal health request failed"))
    }
}
