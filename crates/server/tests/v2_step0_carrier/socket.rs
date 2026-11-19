//! Bounded transport and enrollment handshake for the disposable carrier.
use super::{Enrollment, Envelope, Frame, Hello, Message, PACKAGES, Welcome, wire};
use calternal_plugin_host::encode_frame;
use calternal_types::SpaceId;
use std::{collections::VecDeque, net::SocketAddr, os::fd::AsRawFd, sync::Arc};
use tokio::{
    io::BufReader,
    net::{
        TcpListener, TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
};
use uuid::Uuid;

pub(super) struct Socket {
    input: BufReader<OwnedReadHalf>,
    line: Vec<u8>,
    pub(super) output: OwnedWriteHalf,
    pub(super) welcome: Welcome,
    tx: u64,
    rx: u64,
    credit: [usize; 2],
    digest: Arc<str>,
    controls: VecDeque<Envelope>,
    control_bytes: usize,
    data: [VecDeque<(Envelope, usize)>; 2],
    next: usize,
}
impl Socket {
    fn new(stream: TcpStream, welcome: Welcome, digest: Arc<str>) -> Self {
        let (read, output) = stream.into_split();
        Self {
            input: BufReader::with_capacity(4096, read),
            line: Vec::new(),
            output,
            credit: [welcome.credit; 2],
            welcome,
            tx: 0,
            rx: 0,
            digest,
            controls: VecDeque::new(),
            control_bytes: 0,
            data: Default::default(),
            next: 0,
        }
    }

    pub(super) fn enqueue(&mut self, package: usize, ticket: &str, message: Message) {
        let is_data = matches!(message, Message::Child { .. } | Message::Fill { .. });
        let envelope = Envelope {
            v: 1,
            epoch: self.welcome.epoch,
            seq: u64::MAX,
            package: PACKAGES[package].into(),
            digest: self.digest.to_string(),
            ticket: ticket.into(),
            message,
        };
        let reserved = wire::encode(&envelope).len();
        if is_data {
            assert!(self.data[package].len() < 32 && reserved <= self.credit[package]);
            self.credit[package] -= reserved;
            self.data[package].push_back((envelope, reserved));
        } else {
            assert!(
                self.controls.len() < 16 && self.control_bytes + reserved <= wire::CONTROL_CREDIT
            );
            self.control_bytes += reserved;
            self.controls.push_back(envelope);
        }
    }

    pub(super) fn kernel_buffers(&self) -> (i32, i32) {
        let size = |option| {
            let mut bytes: libc::c_int = 0;
            let mut len = std::mem::size_of_val(&bytes) as libc::socklen_t;
            let rc = unsafe {
                libc::getsockopt(
                    self.output.as_ref().as_raw_fd(),
                    libc::SOL_SOCKET,
                    option,
                    (&raw mut bytes).cast(),
                    &mut len,
                )
            };
            assert_eq!(rc, 0);
            bytes
        };
        (size(libc::SO_SNDBUF), size(libc::SO_RCVBUF))
    }

    pub(super) fn next_sequence(&self) -> u64 {
        self.tx + 1
    }

    pub(super) fn dequeue(&mut self) -> Option<Vec<u8>> {
        let (mut envelope, debit) = if let Some(control) = self.controls.pop_front() {
            self.control_bytes -= wire::encode(&control).len();
            (control, None)
        } else {
            let first = self.next;
            let package = if self.data[first].is_empty() {
                1 - first
            } else {
                first
            };
            let (value, reserved) = self.data[package].pop_front()?;
            self.next = 1 - package;
            (value, Some((package, reserved)))
        };
        self.tx += 1;
        envelope.seq = self.tx;
        let bytes = wire::encode(&envelope);
        if let Some((package, reserved)) = debit {
            self.credit[package] += reserved - bytes.len();
        }
        Some(bytes)
    }

    pub(super) async fn flush_one(&mut self) -> std::io::Result<()> {
        let bytes = self.dequeue().expect("one queued frame");
        wire::send(&mut self.output, &bytes).await
    }

    pub(super) async fn write(
        &mut self,
        package: usize,
        ticket: &str,
        message: Message,
    ) -> std::io::Result<()> {
        self.enqueue(package, ticket, message);
        self.flush_one().await
    }

    pub(super) async fn read(&mut self) -> std::io::Result<(usize, Envelope, usize)> {
        wire::line(&mut self.input, &mut self.line).await?;
        let bytes = std::mem::take(&mut self.line);
        let invalid = || std::io::Error::from(std::io::ErrorKind::InvalidData);
        if !wire::strict(&bytes) {
            return Err(invalid());
        }
        let e: Envelope = serde_json::from_slice(&bytes).map_err(std::io::Error::other)?;
        if e.epoch != self.welcome.epoch
            || e.seq != self.rx + 1
            || e.v != 1
            || e.digest != *self.digest
        {
            return Err(invalid());
        }
        let package = PACKAGES
            .iter()
            .position(|name| *name == e.package)
            .ok_or_else(invalid)?;
        self.rx += 1;
        if let Message::Credit { bytes } = e.message {
            self.credit[package] = self.credit[package]
                .checked_add(bytes)
                .ok_or_else(invalid)?;
            if self.credit[package] > self.welcome.credit {
                return Err(invalid());
            }
        }
        Ok((package, e, bytes.len()))
    }

    pub(super) async fn child(&mut self, package: usize, frame: Frame) {
        self.write_child(package, frame).await.unwrap();
    }

    pub(super) async fn write_child(
        &mut self,
        package: usize,
        frame: Frame,
    ) -> std::io::Result<()> {
        let ticket = frame.invocation_id.clone();
        let value = serde_json::from_slice(&encode_frame(&frame).unwrap()).unwrap();
        self.write(package, &ticket, Message::Child { frame: value })
            .await
    }
}

fn configure(socket: &TcpStream) {
    socket.set_nodelay(true).unwrap();
    let size: libc::c_int = 4096;
    for option in [libc::SO_RCVBUF, libc::SO_SNDBUF] {
        // Supply a native int of exactly the option's required length.
        assert_eq!(
            unsafe {
                libc::setsockopt(
                    socket.as_raw_fd(),
                    libc::SOL_SOCKET,
                    option,
                    (&raw const size).cast(),
                    std::mem::size_of_val(&size) as libc::socklen_t,
                )
            },
            0
        );
    }
}

async fn accepted(listener: &TcpListener, enrollment: &Enrollment, digest: Arc<str>) -> Socket {
    let (mut stream, _) = listener.accept().await.unwrap();
    configure(&stream);
    let mut reader = BufReader::new(&mut stream);
    let mut line = Vec::new();
    wire::line(&mut reader, &mut line).await.unwrap();
    assert!(wire::strict(&line));
    let hello: Hello = serde_json::from_slice(&line).unwrap();
    let service = enrollment.service.clone();
    let auth = enrollment.auth.clone();
    let actor = enrollment.authority.account_id;
    let id = tokio::task::spawn_blocking(move || {
        let id = service.authenticate(&hello.token).unwrap();
        assert!(auth.capabilities(actor, hello.space).unwrap().is_some());
        id
    })
    .await
    .unwrap();
    assert_eq!(id, enrollment.id);
    assert!(enrollment.allowed.contains(&hello.space));
    assert_eq!(hello.v, 1);
    let welcome = Welcome {
        v: 1,
        space: hello.space,
        epoch: Uuid::new_v4(),
        credit: wire::DATA_CREDIT,
        control: wire::CONTROL_CREDIT,
        lease_ms: wire::LEASE.as_millis(),
    };
    wire::send(&mut stream, &wire::encode(&welcome))
        .await
        .unwrap();
    Socket::new(stream, welcome, digest)
}

async fn open(
    address: SocketAddr,
    enrollment: &Enrollment,
    space: SpaceId,
    digest: Arc<str>,
) -> Socket {
    let mut socket = TcpStream::connect(address).await.unwrap();
    configure(&socket);
    wire::send(
        &mut socket,
        &wire::encode(&Hello {
            token: enrollment.token.clone(),
            space,
            v: 1,
        }),
    )
    .await
    .unwrap();
    let mut reader = BufReader::new(&mut socket);
    let mut line = Vec::new();
    wire::line(&mut reader, &mut line).await.unwrap();
    assert!(wire::strict(&line));
    let welcome: Welcome = serde_json::from_slice(&line).unwrap();
    assert_eq!((welcome.v, welcome.space), (1, space));
    Socket::new(socket, welcome, digest)
}

pub(super) async fn pair(
    listener: &TcpListener,
    enrollment: &Enrollment,
    space: SpaceId,
    digest: Arc<str>,
) -> (Socket, Socket) {
    let remote = open(
        listener.local_addr().unwrap(),
        enrollment,
        space,
        digest.clone(),
    );
    tokio::join!(accepted(listener, enrollment, digest), remote)
}
