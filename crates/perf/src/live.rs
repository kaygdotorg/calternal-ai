//! Bounded SSE reader for write-to-delivery observations, including HTTP acknowledgement.
use crate::http::{Http, ORIGIN, Result};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
pub(crate) struct Live {
    reader: BufReader<reqwest::blocking::Response>,
}
impl Live {
    pub(crate) fn open(h: &Http, path: &str, query: &[(String, String)]) -> Result<Self> {
        let response = h
            .client
            .get(format!("{}{path}", h.base))
            .header("host", "localhost:18080")
            .header("origin", ORIGIN)
            .header("cookie", &h.cookie)
            .query(query)
            .send()?
            .error_for_status()?;
        Ok(Self {
            reader: BufReader::new(response),
        })
    }
    pub(crate) fn next(&mut self) -> Result<Value> {
        loop {
            let mut line = String::new();
            let n = self.reader.by_ref().take(65537).read_line(&mut line)?;
            if n == 0 || n > 65536 {
                return Err("SSE closed or frame exceeded 64 KiB".into());
            }
            if let Some(data) = line.strip_prefix("data: ") {
                return Ok(serde_json::from_str(data.trim())?);
            }
        }
    }
    pub(crate) fn wait_type(&mut self, kind: &str) -> Result<Value> {
        for _ in 0..1024 {
            let v = self.next()?;
            if v["type"] == kind {
                return Ok(v);
            }
        }
        Err("SSE expected event not found within 1024 frames".into())
    }
}
