//! Bounded private SMTP queue. Mail and SMTP credentials never enter diagnostics.

use calternal_auth::AccountMail;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::Mailbox,
    transport::smtp::authentication::Credentials,
};
use std::{io, time::Duration};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub(crate) struct Mailer(mpsc::Sender<Delivery>);

struct Delivery {
    mail: AccountMail,
    completion: Option<oneshot::Sender<bool>>,
}

impl Mailer {
    pub(crate) fn from_environment() -> io::Result<Option<Self>> {
        let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "invalid SMTP configuration");
        let host = match std::env::var("CALTERNAL_SMTP_HOST") {
            Ok(host) if !host.is_empty() => host,
            Ok(_) | Err(std::env::VarError::NotPresent) => return Ok(None),
            Err(_) => return Err(invalid()),
        };
        let setting = |name: &str| {
            std::env::var(name)
                .ok()
                .filter(|s| !s.is_empty())
                .ok_or_else(invalid)
        };
        let from: Mailbox = setting("CALTERNAL_SMTP_FROM")?
            .parse()
            .map_err(|_| invalid())?;
        let login = setting("CALTERNAL_SMTP_USERNAME")?;
        let password = setting("CALTERNAL_SMTP_PASSWORD")?;
        let mut builder = match std::env::var("CALTERNAL_SMTP_TLS").as_deref() {
            Ok("starttls") => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host),
            Ok("implicit") | Err(std::env::VarError::NotPresent) => {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&host)
            }
            _ => return Err(invalid()),
        }
        .map_err(|_| invalid())?;
        if let Ok(port) = std::env::var("CALTERNAL_SMTP_PORT") {
            builder = builder.port(port.parse().map_err(|_| invalid())?);
        }
        let smtp = builder
            .credentials(Credentials::new(login, password))
            .build();
        let (tx, mut rx) = mpsc::channel::<Delivery>(64);
        tokio::spawn(async move {
            while let Some(delivery) = rx.recv().await {
                let sent = if let Some(message) = message(from.clone(), delivery.mail) {
                    tokio::time::timeout(Duration::from_secs(12), smtp.send(message))
                        .await
                        .is_ok_and(|r| r.is_ok())
                } else {
                    false
                };
                if !sent {
                    tracing::warn!("account security mail delivery failed");
                }
                if let Some(result) = delivery.completion {
                    let _ = result.send(sent);
                }
            }
        });
        Ok(Some(Self(tx)))
    }

    pub(crate) async fn challenge(&self, mail: AccountMail) -> bool {
        let (tx, rx) = oneshot::channel();
        if self
            .0
            .try_send(Delivery {
                mail,
                completion: Some(tx),
            })
            .is_err()
        {
            return false;
        }
        tokio::time::timeout(Duration::from_secs(15), rx)
            .await
            .is_ok_and(|r| r.unwrap_or(false))
    }

    pub(crate) fn notice(&self, mail: AccountMail) -> bool {
        self.0
            .try_send(Delivery {
                mail,
                completion: None,
            })
            .is_ok()
    }
}

fn message(from: Mailbox, mail: AccountMail) -> Option<Message> {
    let body = mail.code.map_or_else(
        || "Review recent changes in your Calternal account security settings.".to_owned(),
        |code| format!("Enter this code on the Calternal page where you requested it:\r\n\r\n{code}\r\n\r\nIt expires in 10 minutes."),
    );
    Message::builder()
        .from(from)
        .to(mail.to.parse().ok()?)
        .subject(mail.subject)
        .body(body)
        .ok()
}
