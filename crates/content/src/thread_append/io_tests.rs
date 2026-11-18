use super::*;
use std::io::{Cursor, SeekFrom};

#[derive(Default)]
struct Counted {
    file: Cursor<Vec<u8>>,
    read: usize,
    written: usize,
    writes: usize,
    short: bool,
}

impl Read for Counted {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = self.file.read(out)?;
        self.read += n;
        Ok(n)
    }
}

impl Write for Counted {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        let bytes = if self.short {
            &bytes[..bytes.len() / 2]
        } else {
            bytes
        };
        let n = self.file.write(bytes)?;
        self.written += n;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for Counted {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.file.seek(pos)
    }
}

#[test]
fn append_at_sixteen_mib_reads_one_tail_and_writes_the_whole_record_once() {
    let record = "## Claude\n\nA complete response.\n".as_bytes();
    let mut file = Counted {
        file: Cursor::new(vec![b'x'; 16 * 1024 * 1024 - record.len()]),
        ..Default::default()
    };
    let offset = file.file.get_ref().len() as u64;
    let witness = blake3::hash(&vec![b'x'; WITNESS_BYTES]);
    place(&mut file, offset, offset, witness, record, |_| Ok(())).unwrap();
    assert_eq!(file.read, WITNESS_BYTES);
    assert_eq!((file.writes, file.written), (1, record.len()));
    assert_eq!(&file.file.get_ref()[offset as usize..], record);
    assert!(
        file.file.get_ref()[..offset as usize]
            .iter()
            .all(|&ch| ch == b'x')
    );
}

#[test]
fn recovery_completes_matching_torn_suffixes_at_multiple_cuts() {
    let prefix = "# Talk\n\nEarlier committed output.\n".as_bytes();
    let record = "## Claude\n\nA Unicode response 💬.\n".as_bytes();
    for cut in [0, 1, 4, 29, record.len() - 1, record.len()] {
        let mut file = Counted {
            file: Cursor::new([prefix, &record[..cut]].concat()),
            ..Default::default()
        };
        let len = file.file.get_ref().len() as u64;
        place(
            &mut file,
            len,
            prefix.len() as u64,
            blake3::hash(prefix),
            record,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(file.file.get_ref(), &[prefix, record].concat(), "cut {cut}");
        assert_eq!(file.writes, usize::from(cut < record.len()));
        assert_eq!(file.written, record.len() - cut);
    }
}

#[test]
fn a_short_write_is_not_reported_as_success_or_retried_inline() {
    let prefix = b"Earlier bytes.\n";
    let record = b"New record.\n";
    let mut file = Counted {
        file: Cursor::new(prefix.to_vec()),
        short: true,
        ..Default::default()
    };
    let error = place(
        &mut file,
        prefix.len() as u64,
        prefix.len() as u64,
        blake3::hash(prefix),
        record,
        |_| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::WriteZero);
    assert_eq!(file.writes, 1);
    assert_eq!(&file.file.get_ref()[..prefix.len()], prefix);
}

#[test]
fn unexpected_tail_or_torn_bytes_are_preserved() {
    for current in [b"altered!\npre".as_slice(), b"earlier.\nXX".as_slice()] {
        let prefix = b"earlier.\n";
        let mut file = Counted {
            file: Cursor::new(current.to_vec()),
            ..Default::default()
        };
        let error = place(
            &mut file,
            current.len() as u64,
            prefix.len() as u64,
            blake3::hash(prefix),
            b"prepared record\n",
            |_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(file.writes, 0);
        assert_eq!(file.file.get_ref(), current);
    }
}
