//! Bounded, direct IMAPS transport. Only provider-approved TLS can receive a login.
use super::Action;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use calternal_plugin_host::Error;
use imap::types::{Fetch, Flag, NameAttribute};
use imap_proto::types::{BodyContentCommon, BodyParams, BodyStructure, SectionPath};
use mailparse::MailHeaderMap;
use openssl::ssl::{SslConnector, SslMethod, SslStream};
use serde_json::{Value, json};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

const PAGE: u32 = 20;
const MAX_PART: usize = 16 * 1024;
const MAX_MIME: usize = 4096;
const MAX_RESPONSE: usize = 256 * 1024;

pub(super) fn safe_string(s: &str) -> bool {
    !s.is_empty() && s.len() <= 320 && !s.chars().any(char::is_control)
}

impl Action {
    pub(super) fn validate(&self) -> Result<(), Error> {
        let valid = match self {
            Self::RequestPassword | Self::Folders => true,
            Self::Headers { folder, before, uid_validity } => {
                safe_string(folder)
                    && match (before, uid_validity) {
                        (Some(before), Some(validity)) => *before > 1 && *validity > 0,
                        (None, None) => true,
                        _ => false,
                    }
            }
            Self::Read { folder, uid_validity, uid } | Self::MarkRead { folder, uid_validity, uid } =>
                safe_string(folder) && *uid_validity > 0 && *uid > 0,
        };
        valid.then_some(()).ok_or(Error::Invalid)
    }
}

pub(super) struct Socket {
    stream: TcpStream,
    deadline: Instant,
    remaining: usize,
}

impl Socket {
    fn timeout(&self) -> io::Result<Duration> {
        self.deadline.checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
            .map(|left| left.min(Duration::from_secs(2)))
            .ok_or_else(|| io::ErrorKind::TimedOut.into())
    }
}

impl Read for Socket {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.stream.set_read_timeout(Some(self.timeout()?))?;
        if self.remaining == 0 {
            return Err(io::Error::other("IMAPS response limit"));
        }
        let size = bytes.len().min(self.remaining);
        let read = self.stream.read(&mut bytes[..size])?;
        self.remaining -= read;
        Ok(read)
    }
}

impl Write for Socket {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.stream.set_write_timeout(Some(self.timeout()?))?;
        self.stream.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

pub(super) fn connect(host: &str, addresses: &[SocketAddr]) -> Result<SslStream<Socket>, Error> {
    let connector = SslConnector::builder(SslMethod::tls_client()).map_err(|_| Error::Refused)?;
    dial(&connector.build(), host, addresses)
}

fn dial(connector: &SslConnector, host: &str, addresses: &[SocketAddr]) -> Result<SslStream<Socket>, Error> {
    let deadline = Instant::now() + Duration::from_secs(8);
    let stream = addresses.iter().take(2).find_map(|address| {
        TcpStream::connect_timeout(address, Duration::from_secs(1)).ok()
    }).ok_or(Error::Refused)?;
    let socket = Socket { stream, deadline, remaining: MAX_RESPONSE };
    connector.connect(host, socket).map_err(|_| Error::Refused)
}

// Limit nesting before the IMAP library parses server-supplied BODYSTRUCTURE.
// The filter ignores quoted strings and counted literals, which can contain
// arbitrary parentheses. All checks happen before bytes reach the parser.
#[derive(Default)]
struct ResponseSyntax {
    depth: u8,
    quote: bool,
    escape: bool,
    literal: Option<usize>,
    literal_closed: bool,
    remaining: usize,
}

impl ResponseSyntax {
    fn accept(&mut self, bytes: &[u8]) -> io::Result<()> {
        for &byte in bytes {
            if self.remaining > 0 {
                self.remaining -= 1;
                continue;
            }
            if self.quote {
                if self.escape {
                    self.escape = false;
                } else if byte == b'\\' {
                    self.escape = true;
                } else if byte == b'"' || byte == b'\n' {
                    self.quote = false;
                }
                continue;
            }
            if let Some(len) = self.literal {
                match byte {
                    b'0'..=b'9' if !self.literal_closed => {
                        self.literal = len.checked_mul(10).and_then(|n| n.checked_add((byte - b'0') as usize));
                        continue;
                    }
                    b'}' if !self.literal_closed => { self.literal_closed = true; continue; }
                    b'\r' if self.literal_closed => continue,
                    b'\n' if self.literal_closed => {
                        if len > MAX_RESPONSE { return Err(io::Error::other("IMAPS literal limit")); }
                        self.remaining = len;
                        self.literal = None;
                        continue;
                    }
                    _ => self.literal = None,
                }
            }
            match byte {
                b'"' => self.quote = true,
                b'{' => { self.literal = Some(0); self.literal_closed = false; }
                b'(' => {
                    self.depth += 1;
                    if self.depth > 32 { return Err(io::Error::other("IMAPS nesting limit")); }
                }
                b')' => self.depth = self.depth.saturating_sub(1),
                b'\n' => self.depth = 0,
                _ => {}
            }
        }
        Ok(())
    }
}

struct ResponseStream<S>(S, ResponseSyntax);
impl<S: Read> Read for ResponseStream<S> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.0.read(bytes)?;
        self.1.accept(&bytes[..count])?;
        Ok(count)
    }
}
impl<S: Write> Write for ResponseStream<S> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.0.write(bytes) }
    fn flush(&mut self) -> io::Result<()> { self.0.flush() }
}

fn refused<T>(result: Result<T, imap::Error>) -> Result<T, Error> {
    // A provider can echo login commands in errors. Drop all provider diagnostics.
    result.map_err(|_| Error::Refused)
}

pub(super) fn execute<S: Read + Write>(
    stream: S, username: &str, password: &[u8], action: Action,
) -> Result<Value, Error> {
    let password = std::str::from_utf8(password).map_err(|_| Error::Invalid)?;
    if password.is_empty() || password.len() > 1024 || password.chars().any(char::is_control) {
        return Err(Error::Invalid);
    }
    let mut client = imap::Client::new(ResponseStream(stream, ResponseSyntax::default()));
    let greeting = refused(client.read_greeting())?;
    if !greeting.starts_with(b"* OK ") {
        return Err(Error::Refused);
    }
    let mut session = client.login(username, password).map_err(|_| Error::Refused)?;
    match action {
        Action::RequestPassword => Err(Error::Invalid),
        Action::Folders => {
            let names = refused(session.list(None, Some("\"*\"")))?;
            let folders: Vec<_> = names.iter()
                .filter(|n| safe_string(n.name()))
                .take(200)
                .map(|name| json!({
                    "name": name.name(),
                    "delimiter": name.delimiter().filter(|d| d.len() <= 8),
                    "selectable": !name.attributes().contains(&NameAttribute::NoSelect),
                })).collect();
            Ok(json!({ "folders": folders, "truncated": folders.len() < names.len() }))
        }
        Action::Headers { folder, before, uid_validity } => {
            let mailbox = refused(session.examine(&folder))?;
            let validity = mailbox.uid_validity.filter(|n| *n > 0).ok_or(Error::Refused)?;
            if uid_validity.is_some_and(|n| n != validity) { return Err(Error::Refused); }
            let last = before.map_or(mailbox.exists, |n| (n - 1).min(mailbox.exists));
            if last == 0 {
                return Ok(json!({ "headers": [], "next_before": null, "uid_validity": validity }));
            }
            let first = last.saturating_sub(PAGE - 1).max(1);
            let found = refused(session.fetch(format!("{first}:{last}"), "(UID FLAGS ENVELOPE RFC822.SIZE)"))?;
            let mut messages: Vec<_> = found.iter()
                .filter(|item| (first..=last).contains(&item.message) && item.uid.is_some() && item.envelope().is_some())
                .collect();
            messages.sort_unstable_by_key(|item| std::cmp::Reverse(item.message));
            messages.dedup_by_key(|item| item.message);
            Ok(json!({
                "headers": messages.iter().map(|message| header(message)).collect::<Vec<_>>(),
                "next_before": (first > 1).then_some(first), "uid_validity": validity
            }))
        }
        Action::Read { folder, uid_validity, uid } => {
            let mailbox = refused(session.examine(&folder))?;
            if mailbox.uid_validity != Some(uid_validity) { return Err(Error::Refused); }
            read_message(&mut session, uid)
        }
        Action::MarkRead { folder, uid_validity, uid } => {
            let mailbox = refused(session.select(&folder))?;
            if mailbox.uid_validity != Some(uid_validity) { return Err(Error::Refused); }
            refused(session.uid_store(uid.to_string(), "+FLAGS.SILENT (\\Seen)"))?;
            Ok(json!({ "read": true }))
        }
    }
}

fn envelope_text(bytes: Option<&[u8]>, limit: usize) -> Option<String> {
    let bytes = bytes?;
    // Decode RFC 2047 encoded words through the MIME parser without injecting
    // a second header. Plain UTF-8 and ISO-8859-1 fallback are handled there too.
    let mut line = b"Field: ".to_vec();
    line.extend(bytes.iter().take(4096).map(|b| if b.is_ascii_control() { b' ' } else { *b }));
    Some(mailparse::parse_header(&line).ok()?.0.get_value().chars().take(limit).collect())
}

fn addresses(values: Option<&[imap_proto::types::Address<'_>]>) -> Vec<String> {
    values.unwrap_or_default().iter().take(4).filter_map(|address| {
        let local = envelope_text(address.mailbox, 80)?;
        let domain = envelope_text(address.host, 100)?;
        Some(format!("{local}@{domain}"))
    }).collect()
}

fn header(fetch: &Fetch) -> Value {
    let envelope = fetch.envelope();
    json!({
        "uid": fetch.uid,
        "subject": envelope_text(envelope.and_then(|e| e.subject), 160),
        "from": addresses(envelope.and_then(|e| e.from.as_deref())),
        "date": envelope_text(envelope.and_then(|e| e.date), 64),
        "read": fetch.flags().contains(&Flag::Seen),
        "bytes": fetch.size,
    })
}

#[derive(Default)]
struct Parts {
    plain: Option<(Vec<u32>, u32)>,
    html: Option<(Vec<u32>, u32)>,
    attachments: Vec<Value>,
    count: usize,
}

fn parameter<'a>(params: &'a BodyParams<'a>, key: &str) -> Option<&'a str> {
    params.as_ref()?.iter().find_map(|(name, value)| name.eq_ignore_ascii_case(key).then_some(*value))
}

fn filename<'a>(part: &'a BodyContentCommon<'a>) -> Option<&'a str> {
    part.disposition.as_ref().and_then(|d| parameter(&d.params, "filename"))
        .or_else(|| parameter(&part.ty.params, "name"))
}

fn disposition_attachment(part: &BodyContentCommon<'_>) -> bool {
    part.disposition.as_ref().is_some_and(|d| d.ty.eq_ignore_ascii_case("attachment"))
        || filename(part).is_some()
}

impl Parts {
    fn visit(&mut self, body: &BodyStructure<'_>, path: &mut Vec<u32>) -> Result<(), Error> {
        self.count += 1;
        if self.count > 100 || path.len() > 12 { return Err(Error::Refused); }
        let (common, size) = match body {
            BodyStructure::Multipart { common, bodies, .. } if !disposition_attachment(common) => {
                for (i, part) in bodies.iter().enumerate() {
                    path.push(i as u32 + 1);
                    self.visit(part, path)?;
                    path.pop();
                }
                return Ok(());
            }
            BodyStructure::Text { common, other, .. } if !disposition_attachment(common)
                && (common.ty.subtype.eq_ignore_ascii_case("plain") || common.ty.subtype.eq_ignore_ascii_case("html")) =>
            {
                let slot = if common.ty.subtype.eq_ignore_ascii_case("plain") { &mut self.plain } else { &mut self.html };
                if slot.is_none() {
                    *slot = Some((if path.is_empty() { vec![1] } else { path.clone() }, other.octets));
                }
                return Ok(());
            }
            BodyStructure::Multipart { common, .. } => (common, None),
            BodyStructure::Text { common, other, .. } | BodyStructure::Basic { common, other, .. }
            | BodyStructure::Message { common, other, .. } => (common, Some(other.octets)),
        };
        self.attachments.push(json!({
            "filename": filename(common).map(|name| name.chars().take(160).collect::<String>()),
            "media_type": format!("{}/{}", common.ty.ty, common.ty.subtype).chars().take(96).collect::<String>(),
            "bytes": size
        }));
        Ok(())
    }
}

fn read_message<S: Read + Write>(
    session: &mut imap::Session<S>, uid: u32,
) -> Result<Value, Error> {
    let uid_text = uid.to_string();
    let found = refused(session.uid_fetch(&uid_text, "(UID FLAGS ENVELOPE RFC822.SIZE BODYSTRUCTURE)"))?;
    let message = found.iter().find(|response| response.uid == Some(uid)).ok_or(Error::Refused)?;
    let body = message.bodystructure().ok_or(Error::Refused)?;
    let whole_message = !matches!(body, BodyStructure::Multipart { .. });
    let mut parts = Parts::default();
    parts.visit(body, &mut Vec::new())?;
    let text = read_section(session, &uid_text, uid, parts.plain, whole_message)?;
    let html = read_section(session, &uid_text, uid, parts.html, whole_message)?;
    let header = header(message);
    let eml = snapshot(&header, message.envelope(), text.as_deref(), html.as_deref());
    // The worker persists EML for the sealed renderer. It receives no attachment bytes.
    Ok(json!({ "header": header, "text": text, "has_html": html.is_some(),
        "attachments": parts.attachments, "eml": eml }))
}

fn read_section<S: Read + Write>(
    session: &mut imap::Session<S>, uid_text: &str, uid: u32,
    part: Option<(Vec<u32>, u32)>, whole_message: bool,
) -> Result<Option<String>, Error> {
    use imap_proto::types::MessageSection;
    let Some((path, size)) = part else { return Ok(None); };
    if size as usize > MAX_PART { return Err(Error::Refused); }
    let number = path.iter().map(u32::to_string).collect::<Vec<_>>().join(".");
    let mime = if whole_message { "HEADER".to_string() } else { format!("{number}.MIME") };
    let query = format!("(UID BODY.PEEK[{mime}]<0.{}> BODY.PEEK[{number}]<0.{}>)", MAX_MIME + 1, MAX_PART + 1);
    let result = refused(session.uid_fetch(uid_text, query))?;
    let response = result.iter().find(|row| row.uid == Some(uid)).ok_or(Error::Refused)?;
    let header = if whole_message { response.header() } else {
        response.section(&SectionPath::Part(path.clone(), Some(MessageSection::Mime)))
    }.filter(|bytes| bytes.len() <= MAX_MIME && bytes.ends_with(b"\r\n\r\n")).ok_or(Error::Refused)?;
    let bytes = response.section(&SectionPath::Part(path, None))
        .filter(|bytes| bytes.len() <= MAX_PART).ok_or(Error::Refused)?;
    let input = [header, bytes].concat();
    let parsed = mailparse::parse_mail(&input).map_err(|_| Error::Refused)?;
    if !parsed.ctype.mimetype.eq_ignore_ascii_case("text/plain")
        && !parsed.ctype.mimetype.eq_ignore_ascii_case("text/html") { return Err(Error::Refused); }
    let decoded = parsed.get_body().map_err(|_| Error::Refused)?;
    if decoded.len() > MAX_PART { return Err(Error::Refused); }
    Ok(Some(decoded))
}

fn snapshot(header: &Value, envelope: Option<&imap_proto::types::Envelope<'_>>, text: Option<&str>, html: Option<&str>) -> String {
    let mut eml = String::new();
    for (name, raw) in [("Date", envelope.and_then(|e| e.date)), ("Subject", envelope.and_then(|e| e.subject)),
        ("Message-ID", envelope.and_then(|e| e.message_id))] {
        if let Some(raw) = raw.filter(|s| s.len() <= 900 && s.iter().all(|b| !b.is_ascii_control())) {
            eml.push_str(&format!("{name}: {}\r\n", String::from_utf8_lossy(raw)));
        }
    }
    for address in header["from"].as_array().into_iter().flatten().filter_map(Value::as_str).take(1) {
        if address.bytes().all(|b| b.is_ascii_alphanumeric() || b"@+.-_".contains(&b)) {
            eml.push_str(&format!("From: {address}\r\n"));
        }
    }
    eml.push_str("MIME-Version: 1.0\r\nX-Calternal-Snapshot: inline-text-only\r\nContent-Type: multipart/alternative; boundary=\"calternal-mail-snapshot\"\r\n\r\n");
    for (mime, part) in [("text/plain", text.or_else(|| html.is_none().then_some(""))), ("text/html", html)] {
        if let Some(part) = part {
            eml.push_str(&format!("--calternal-mail-snapshot\r\nContent-Type: {mime}; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n"));
            for line in STANDARD.encode(part).as_bytes().chunks(76) {
                // Standard base64 encodes to ASCII only.
                eml.push_str(std::str::from_utf8(line).expect("base64 ASCII"));
                eml.push_str("\r\n");
            }
        }
    }
    eml.push_str("--calternal-mail-snapshot--\r\n");
    eml
}

pub(in crate::plugins) fn snapshot_html(bytes: &[u8]) -> Option<String> {
    // Read only our generated inline representation. No recursion into attachments.
    if bytes.len() > MAX_PART * 4 { return None; }
    let (headers, _) = mailparse::parse_headers(bytes).ok()?;
    if headers.get_first_value("X-Calternal-Snapshot").as_deref() != Some("inline-text-only") { return None; }
    let mail = mailparse::parse_mail(bytes).ok()?;
    let part = mail.subparts.iter().find(|part| part.ctype.mimetype == "text/html")?;
    part.get_body().ok().filter(|html| html.len() <= MAX_PART)
}
