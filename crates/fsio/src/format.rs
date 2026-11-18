//! Format tags shared by Calternal-owned file envelopes.

/// Reject an unknown version of the expected family with `Unsupported`.
/// A missing or malformed tag and a different family are invalid data. Reads
/// only two short strings, with no allocation on the supported path.
pub fn require(actual: &str, supported: &str) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    if actual == supported {
        return Ok(());
    }
    let invalid = || Error::new(ErrorKind::InvalidData, "missing or invalid file format");
    let (family, version) = actual.rsplit_once("/v").ok_or_else(invalid)?;
    let (expected, _) = supported.rsplit_once("/v").ok_or_else(invalid)?;
    if family == expected && version.parse::<u32>().is_ok() {
        Err(Error::new(
            ErrorKind::Unsupported,
            "unsupported file format version",
        ))
    } else {
        Err(invalid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    #[test]
    fn recognizes_its_own_family_and_rejects_other_versions() {
        let current = "calternal-example/v1";
        require(current, current).unwrap();
        assert_eq!(
            require("calternal-example/v2", current).unwrap_err().kind(),
            ErrorKind::Unsupported
        );
        assert_eq!(
            require("calternal-other/v1", current).unwrap_err().kind(),
            ErrorKind::InvalidData
        );
    }
}
