//! A TLS IMAP fixture with generated messages and no external dependencies.
use super::*;
use openssl::{
    asn1::Asn1Time,
    hash::MessageDigest,
    pkey::PKey,
    rsa::Rsa,
    ssl::SslAcceptor,
    x509::{X509, X509NameBuilder, extension::SubjectAlternativeName},
};
use std::{
    io::{BufRead, BufReader},
    net::TcpListener,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

const HOST: &str = "generated.mail.test";
const UID_VALIDITY: u32 = 27;
const UID: u32 = 122;
const PLAIN: &str = "Generated plain text.";
const HTML: &str =
    "<p>Generated HTML</p><script>bad()</script><img src=\"https://example.com/one.png\">";
const ENVELOPE: &str = "(NIL \"Generated message\" ((NIL NIL \"test\" \"example.test\")) NIL NIL NIL NIL NIL NIL \"<generated@example.test>\")";
const STRUCTURE: &str = "((\"TEXT\" \"PLAIN\" (\"CHARSET\" \"UTF-8\") NIL NIL \"7BIT\" 21 1 NIL NIL NIL NIL)(\"TEXT\" \"HTML\" (\"CHARSET\" \"UTF-8\") NIL NIL \"7BIT\" 89 1 NIL NIL NIL NIL)(\"APPLICATION\" \"PDF\" (\"NAME\" \"fixture.pdf\") NIL NIL \"BASE64\" 80000000 NIL (\"ATTACHMENT\" (\"FILENAME\" \"fixture.pdf\")) NIL NIL) \"MIXED\" (\"BOUNDARY\" \"fixture\") NIL NIL NIL)";

fn tls() -> (SslAcceptor, SslConnector) {
    let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_text("CN", HOST).unwrap();
    let name = name.build();
    let mut cert = X509::builder().unwrap();
    cert.set_version(2).unwrap();
    cert.set_subject_name(&name).unwrap();
    cert.set_issuer_name(&name).unwrap();
    cert.set_pubkey(&key).unwrap();
    cert.set_not_before(Asn1Time::days_from_now(0).unwrap().as_ref())
        .unwrap();
    cert.set_not_after(Asn1Time::days_from_now(1).unwrap().as_ref())
        .unwrap();
    let san = SubjectAlternativeName::new()
        .dns(HOST)
        .build(&cert.x509v3_context(None, None))
        .unwrap();
    cert.append_extension(san).unwrap();
    cert.sign(&key, MessageDigest::sha256()).unwrap();
    let cert = cert.build();

    let mut accept = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server()).unwrap();
    accept.set_private_key(&key).unwrap();
    accept.set_certificate(&cert).unwrap();
    let mut client = SslConnector::builder(SslMethod::tls_client()).unwrap();
    client.cert_store_mut().add_cert(cert).unwrap();
    (accept.build(), client.build())
}

fn fixture(stream: SslStream<TcpStream>, password: &str, seen: &AtomicBool) {
    let mut stream = BufReader::new(stream);
    stream
        .get_mut()
        .write_all(b"* OK generated IMAP fixture\r\n")
        .unwrap();
    let mut authenticated = false;
    loop {
        let mut line = String::new();
        match stream.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => (),
        }
        let (tag, command) = line.trim_end().split_once(' ').unwrap();
        let mut response = String::new();
        if !authenticated {
            // Never include a LOGIN command or password in errors or assertion output.
            assert!(
                command == format!("LOGIN \"fixture\" \"{password}\""),
                "wrong fixture authentication"
            );
            authenticated = true;
        } else if command == "LIST \"\" \"*\"" {
            response
                .push_str("* LIST () \"/\" \"INBOX\"\r\n* LIST (\\Noselect) \"/\" \"Parent\"\r\n");
        } else if command == "EXAMINE \"INBOX\"" || command == "SELECT \"INBOX\"" {
            response.push_str("* FLAGS (\\Seen)\r\n* 22 EXISTS\r\n* 0 RECENT\r\n* OK [UIDVALIDITY 27] fixture\r\n");
        } else if let Some(command) = command.strip_suffix(" (UID FLAGS ENVELOPE RFC822.SIZE)") {
            let (first, last) = match command {
                "FETCH 3:22" => (3, 22),
                "FETCH 1:2" => (1, 2),
                _ => panic!("unexpected header range"),
            };
            for seq in first..=last {
                response.push_str(&format!(
                    "* {seq} FETCH (UID {} FLAGS () ENVELOPE {ENVELOPE} RFC822.SIZE 80000400)\r\n",
                    seq + 100
                ));
            }
        } else if command == "UID FETCH 122 (UID FLAGS ENVELOPE RFC822.SIZE BODYSTRUCTURE)" {
            let flags = if seen.load(Ordering::Relaxed) {
                "\\Seen"
            } else {
                ""
            };
            response.push_str(&format!("* 22 FETCH (UID 122 FLAGS ({flags}) ENVELOPE {ENVELOPE} RFC822.SIZE 80000400 BODYSTRUCTURE {STRUCTURE})\r\n"));
        } else if let Some(section) = command.strip_prefix("UID FETCH 122 (UID BODY.PEEK[") {
            // This allow-list is the fixture assertion that the attachment never transfers.
            let (seq, mime, body) = match section {
                "1.MIME]<0.4097> BODY.PEEK[1]<0.16385>)" => (1, "plain", PLAIN),
                "2.MIME]<0.4097> BODY.PEEK[2]<0.16385>)" => (2, "html", HTML),
                _ => panic!("read requested an unrelated MIME section"),
            };
            let mime = format!("Content-Type: text/{mime}; charset=UTF-8\r\n\r\n");
            response.push_str(&format!("* 22 FETCH (UID 122 BODY[{seq}.MIME]<0> {{{}}}\r\n{mime} BODY[{seq}]<0> {{{}}}\r\n{body})\r\n", mime.len(), body.len()));
        } else if command == "UID STORE 122 +FLAGS (\\Seen)" {
            seen.store(true, Ordering::Relaxed);
            response.push_str("* 22 FETCH (UID 122 FLAGS (\\Seen))\r\n");
        } else {
            panic!("operation outside fixture commands");
        }
        response.push_str(&format!("{tag} OK complete\r\n"));
        stream.get_mut().write_all(response.as_bytes()).unwrap();
    }
}

#[test]
fn generated_imaps_lists_pages_reads_text_without_attachments_and_marks_seen() {
    let (acceptor, connector) = tls();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let password = uuid::Uuid::new_v4().to_string();
    let fixture_password = password.clone();
    let server = thread::spawn(move || {
        let seen = AtomicBool::new(false);
        for _ in 0..7 {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(4)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(4)))
                .unwrap();
            fixture(acceptor.accept(stream).unwrap(), &fixture_password, &seen);
        }
        assert!(seen.load(Ordering::Relaxed));
    });
    let run = |action| {
        execute(
            dial(&connector, HOST, &[address]).unwrap(),
            "fixture",
            password.as_bytes(),
            action,
        )
    };
    let folder = || "INBOX".to_string();
    let folders = run(Action::Folders).unwrap();
    assert_eq!(folders["folders"][0]["name"], "INBOX");
    assert_eq!(folders["folders"][1]["selectable"], false);
    let first = run(Action::Headers {
        folder: folder(),
        before: None,
        uid_validity: None,
    })
    .unwrap();
    assert_eq!(first["headers"].as_array().unwrap().len(), 20);
    assert_eq!(first["headers"][0]["uid"], UID);
    let last = run(Action::Headers {
        folder: folder(),
        before: Some(first["next_before"].as_u64().unwrap() as u32),
        uid_validity: Some(UID_VALIDITY),
    })
    .unwrap();
    assert_eq!(last["headers"].as_array().unwrap().len(), 2);
    assert_eq!(last["next_before"], Value::Null);
    let read = || {
        run(Action::Read {
            folder: folder(),
            uid_validity: UID_VALIDITY,
            uid: UID,
        })
        .unwrap()
    };
    let message = read();
    assert_eq!(message["text"], PLAIN);
    assert_eq!(message["header"]["read"], false);
    assert_eq!(
        message["attachments"],
        json!([{ "filename": "fixture.pdf", "media_type": "APPLICATION/PDF", "bytes": 80000000 }])
    );
    let eml = message["eml"].as_str().unwrap();
    assert!(!eml.contains("fixture.pdf"));
    assert_eq!(snapshot_html(eml.as_bytes()).as_deref(), Some(HTML));
    let marked = run(Action::MarkRead {
        folder: folder(),
        uid_validity: UID_VALIDITY,
        uid: UID,
    })
    .unwrap();
    assert_eq!(marked, json!({ "read": true }));
    assert_eq!(read()["header"]["read"], true);
    assert!(
        run(Action::MarkRead {
            folder: folder(),
            uid_validity: 99,
            uid: UID
        })
        .is_err()
    );
    server.join().unwrap();
}

#[test]
fn an_untrusted_tls_identity_never_reaches_the_imap_greeting() {
    let (acceptor, connector) = tls();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        assert!(acceptor.accept(stream).is_err());
    });
    assert!(dial(&connector, "other.mail.test", &[address]).is_err());
    server.join().unwrap();
}

#[test]
fn malicious_mime_nesting_is_refused_before_the_protocol_parser() {
    let mut syntax = ResponseSyntax::default();
    syntax.accept(b"* FETCH (BODY[1] {4}\r\n(((").unwrap();
    syntax.accept(b"()\r\n").unwrap();
    assert!(syntax.accept(&[b'('; 33]).is_err());
}

#[tokio::test]
async fn production_resolution_refuses_loopback() {
    assert!(
        crate::plugins::network::fetch::Service::default().mail_addresses("127.0.0.1", 993)
            .await
            .is_err()
    );
}
