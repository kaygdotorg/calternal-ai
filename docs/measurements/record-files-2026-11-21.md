# Record files: append, edit, redact and read on large chat files · 2026-11-21

Status: measurement spike for issue #107 (branch `lane/spike-records`). Throwaway
benchmark under `perf/spikes/record-files/`; nothing here is product code.

Question: in the primitives-only host design (`docs/proposals/primitives-only-host-2026-11-21.md`
and `docs/proposals/primitives/files-and-questions.md` on `origin/lane/v2-host-base`),
a chat is one Markdown file of records. Does an append really cost "2 indexed
reads and 2 flushes whatever the file's length", and does an in-place edit cost
so much that long chats must be split into monthly segments (fork D5 / Q20)?

## Headline

Measured on `calternal-perf-01-backup01` (ext4 on the SSD root, warm page
cache) under the perf lock, 200 samples per cell. One fdatasync there takes
about 7 ms.

- **Append is flat.** p50 is 13.0–13.8 ms and p99 is 20–31 ms at 1, 16, 64 and
  256 MiB (4,400 to 1.1 million messages), in every layout.
- **The "2 indexed reads, 2 flushes" claim is true for append**, counted with
  strace: 2 SQL lookups, 1 fsync of the ledger WAL and 1 fdatasync of the chat
  file. A SQLite checkpoint adds one more flush about once every 12 appends
  (2.08–2.12 flushes on average). Two things do grow slowly with the chat:
  bytes written per append (36 KiB at 1 MiB, 70 KiB at 256 MiB, all SQLite WAL
  pages) and page reads from the OS cache (0 at 1 MiB, 5.6 at 256 MiB).
- **Editing or redacting a recent record is flat** in every layout: 14 ms in A
  and C, 10 ms in B; 24–28 ms for a redaction with its history scrub.
- **Only edits that grow an old record grow with the file.** A random record at
  256 MiB costs 2.0 s p50 and 4.2 s p99 in layout A. About half of that is the
  ledger shifting the offsets of every later record, and half is rewriting the
  file's tail. Rewriting the whole file through `fsio::write_atomic` (A2, what
  "an edit costs a rewrite" means today) costs 4.5 s, and 3.3 s even for a
  recent record.
- **Writing a record in place in its own slot is flat:** about 10 ms for an
  edit and 24 ms for a redaction at every size (B). A redaction always shrinks
  the record, so it can always be done in place.
- **Monthly segments (C) make old-record edits flat** (50–80 ms; a segment is
  about 4 MiB at 600 messages a day). They are also the only layout where a
  foreign editor's save costs little: one typo saved by another editor makes
  the host re-read the whole file, which takes 2.2 s at 256 MiB in A and 72 ms
  in C.

**Recommendation:** drop monthly segments from the host. Append, reads and
recent edits do not need them, and redaction does not need them if the host
writes every shrinking change in place, padded to the old length. Segments only
help with growing edits of old records and with foreign edits of very large
files. Both are rare and bounded (about 16 ms per MiB of file after the edited record, ledger shift and file rewrite together).
Chat's 48-hour edit window keeps Chat away from them entirely. Details in
section 6.

## 1. What was built

One binary, `record-files-spike` (`perf/spikes/record-files/src`, about 1,100 lines):

- **Record file.** Frontmatter (`id`, `kind: chats`, `format: calternal-chat/v2`),
  then records. Each record is a host-written header line
  `**Lee** · 2026-11-20 17:02 ^k2m8q1w4zre7k3f9`, a blank line, the body, and a
  blank line. Anchors are 16 Crockford characters (80 random bits, the ADR 0056
  shape). A body line that looks like a header is escaped with `\` on write.
- **Ledger** (`ledger.db`, host state; WAL, `synchronous=FULL`, `secure_delete=ON`):
  `ledger(seq, anchor UNIQUE, file, author, plugin, at, hash, off, len, rev, state)`
  with an index on `(file, off)`; `versions(seq, rev, at, author, body)` (every
  version of every record, the current one included); `receipts(id)` for request
  IDs; `heads(file, len, last_seq, pending_from)`.
- **Index** (`index.db`, appdata, rebuildable; WAL, `synchronous=NORMAL`): one
  row per record `(seq, file, anchor, author, at, deleted)`. Byte offsets live
  only in the ledger (one place per fact).
- **Corpus.** Deterministic synthetic messages: 66% one short line, 25% a
  paragraph, 6% several paragraphs, 3% a fenced code block; eight authors; one
  message every 30–258 s (about 600 a day), starting 2020-01-01.
- **Edit mix.** 70% a small fix (±1–20 bytes), 20% a sentence added (+30–120
  bytes), 10% rewritten as a new message. "Recent" picks one of the last 500
  records; "random" picks any record uniformly.

### Layouts

| Layout | File shape | Edit or redact writes |
| --- | --- | --- |
| A | one file | the edited record and every byte after it, in place (`fsio::paced::write_all`, 1 MiB steps, each flushed), then truncate and flush |
| A2 | one file | the whole file through `fsio::write_atomic` (temp file, flush, rename, directory flush): what "an edit costs a rewrite" means today |
| B | one file; each record ends with a line of spaces (slack: 25% of the record, at least 16 bytes) | only the record, in place, when the new record fits its slot; otherwise as A, and the record gets fresh slack |
| C | monthly segment files `segments/YYYY-MM.md` (about 18,000 messages, 4 MiB, a month at this rate) | as A, within the record's segment |

### The operations and their crash rule

Write order is ledger before file. Every write commits, in the same SQLite
transaction as its ledger rows, `heads.pending_from` = the first file byte it
may touch. After a crash, recovery regenerates the file from `pending_from` to
the end out of the ledger (latest version of each record, rendered in host
form) and truncates it to `heads.len`. Because the ledger is committed first and
regeneration is idempotent, a crash at any point leaves file and ledger agreeing;
a request that crashed after its commit is present after recovery, and its retry
with the same request ID is answered from `receipts`.

| Operation | Indexed SQL reads | Ledger commit | File |
| --- | --- | --- | --- |
| append | 2 (receipt, head) | rows + version + receipt + head | 1 `stat`, 1 `pwrite` at the expected end, 1 `fdatasync`, 1 `fstat`; then 1 index insert (not synced) |
| edit | 3 (receipt, record by anchor, head) | version + record + offset shift of every later record in the file when the length changes + head | per layout (above) |
| redact | as edit | as edit, plus `DELETE` of the earlier versions (zeroed by `secure_delete`) | per layout; then `wal_checkpoint(TRUNCATE)` so the old words leave the WAL |
| read last 50 | 1 | — | 1 `stat` + 1 ranged read per file touched |
| unread after P | 2 (count, first 50) | — | as above |

**Foreign edits.** Each operation `stat`s the path and compares size, mtime
(ns) and inode with what the host last wrote. On a difference it re-anchors:
reads the file whole, finds every header line, moves each anchor's offset and
length to what the file now says, records a changed body as a new version
stamped `external`, an unknown anchor as an `external` record, and a missing
anchor as gone. The next host write then proceeds normally.

### What was reused and what was written

- Reused: `calternal_fsio::write_atomic` (A2's whole-file replace; creating a new
  segment), `calternal_fsio::paced::write_all` (the repo's rule for big writes,
  #298: 1 MiB steps, each flushed), `calternal_fsio::sync_dir`; `rusqlite` with
  the store's settings (WAL; `synchronous=FULL` for a durable commit, as
  `Commit.durable` does).
- Not reused: `crates/store`'s per-space writer. Its `Commit`, journal and schema
  belong to the content store, and adding a ledger table would mean changing the
  product crate. The spike is single-threaded with one write connection per
  database, which is what the per-space writer is; group commit across
  concurrent appends was therefore not measured.
- Written for the spike: the record format and header parser, the ledger and
  index schema, `pending_from` recovery, re-anchoring, the corpus generator, and
  a driver that counts SQL reads and reads `/proc/self/io` around each sample.

### Correctness checks (dev-01, not timings)

- `crashtest`: 4 layouts × 6 operations (append, append opening a new segment,
  grow and shrink a recent record, grow an old record, redact an old record) × 3
  crash points (after the ledger commit, halfway through the file write, after
  the write and before the flush), by `abort()` in a child process. Then
  `recover` and a byte-for-byte comparison of every file with what the ledger
  renders. **72 of 72 recover to an agreeing file.** (For A2 the "before flush"
  point is inside `fsio::write_atomic`, so those four cases ran to completion
  and also verified.) `abort()` keeps the page cache, so this proves the
  ordering logic, not behaviour under power loss.
- `scrubtest` (A, B, C): a record edited twice with marker words, then redacted.
  Before the redaction the words are in the chat file, `ledger.db` and
  `ledger.db-wal`; after it, **no file in the chat's folder contains either word**.
- Every benchmark case ends with the same byte-for-byte check after all its
  operations, including the foreign edits: all passed (section 3).

## 2. Method

- **Machine:** `calternal-perf-01-backup01`. 4 vCPUs (AMD Ryzen 7 3700X host),
  7 GiB RAM, Debian 13.7, kernel 6.12.115. Work directory on `/dev/sda1`:
  **ext4** (`rw,relatime,discard`) on a non-rotational virtual disk (`sda`,
  rotational 0, so SSD-backed). The HDD (`sdb`, xfs, `/srv/homes`) was not used.
  The job ran under `/usr/local/bin/perf-lock.sh` from 18:55 to 20:32 UTC on
  2026-11-20; no other perf job was waiting or running.
- **Binary:** release build of this branch on `calternal-dev-01-compute02`
  (`nix develop`, rustc 1.99.1, rusqlite 0.41.1 with bundled SQLite). Its
  interpreter was patched to Debian's `/lib64/ld-linux-x86-64.so.2` so it runs
  without the Nix store, then copied to the perf VM. Nothing was built there.
- **Timings** (`perf/spikes/record-files/results/timings.txt`, all samples in
  `results/raw.tsv.gz`): `bench --sizes 1,16,64,256 --layouts A,A2,B,C --samples
  200 --reanchor-samples 20 --history 1000`. Each case builds a fresh chat,
  runs `sync`, then measures. 200 samples per operation and cell; 20 for each
  foreign-edit kind; 1 for "strip trailing whitespace". Wall-clock time per
  operation is measured in-process (`Instant`), with the ledger, file and scrub
  parts timed separately.
  Bytes come from `/proc/self/io` around each sample (`wchar` = bytes handed to
  `write`; `write_bytes` = bytes sent to storage), minus the cost of reading
  `/proc` itself.
- **Counts** (`results/counts.txt`): the same binary in a second, separate run
  under `strace -f -y`, 20 samples per operation (2 per foreign-edit kind). Each
  sample is bracketed by a marker write to `/dev/null`, and `count.py` tallies
  every `read`/`pread64`/`write`/`pwrite64`/`fsync`/`fdatasync`/`stat`/`rename`/
  `ftruncate`/`fadvise64` between the markers, by the file it touched. These
  are counts, not timings; no timing from the traced run is used. "SQL reads"
  are the lookups the code issues (counted in code); "reads" in the strace table
  are system calls (SQLite page misses served by the OS cache, plus file reads).
- **Page cache:** warm. Each chat was just written and the VM has room for it,
  and there is no root to drop caches. Cold reads were not measured.
- **Production differs:** production uses ZFS volumes (`crates/fsio/src/paced.rs`),
  where a flush carries everything written before it. These are ext4 numbers.

## 3. Results

### Corpus

| Size | Records (A, C) | Records (B) | Segments (C) | Ledger | Index |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 MiB | 4,396 | 3,498 | 1 | 1.6 MiB | 0.2 MiB |
| 16 MiB | 68,436 | 54,630 | 4 | 25 MiB | 2.4 MiB |
| 64 MiB | 275,516 | 220,658 | 16 | 101 MiB | 9.6 MiB |
| 256 MiB | 1,103,069 | 882,908 | 61 | 404 MiB | 39 MiB |

The average record is 243 bytes, header included. B's slack holds 20% fewer
records per MiB. **The ledger is about 1.6 times the chat file**, because
recovery needs the current body of every record and so it keeps a full copy.

### Times, flushes and bytes per operation

Times are wall-clock milliseconds. "Ledger" covers the SQL reads and the commit,
"file" covers the record-file write and its flush, and "scrub" covers the
redaction's checkpoint. Bytes are `wchar`; `write_bytes` agrees within 3% and is
in `results/tables.md`.

**append** (ms: p50 / p95 / p99; 200 samples per cell)

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 13.5 / 15.5 / 20.6 | 13.3 / 17.2 / 21.8 | 13.8 / 16.0 / 20.3 |
| 16 MiB | 13.3 / 16.1 / 24.8 | 13.4 / 18.0 / 25.5 | 13.3 / 18.6 / 24.5 |
| 64 MiB | 13.7 / 20.5 / 30.7 | 13.2 / 18.8 / 24.7 | 13.8 / 19.2 / 23.7 |
| 256 MiB | 13.0 / 15.4 / 24.1 | 13.3 / 19.9 / 27.6 | 13.4 / 20.2 / 29.0 |

Split of the mean (ledger ms + file ms [+ scrub ms]), bytes written (wchar) per op:

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 7.06 + 6.73; 36 KiB | 7.06 + 6.74; 35 KiB | 7.10 + 6.77; 36 KiB |
| 16 MiB | 7.09 + 6.75; 49 KiB | 7.21 + 6.85; 48 KiB | 7.19 + 6.76; 49 KiB |
| 64 MiB | 7.64 + 7.07; 64 KiB | 7.29 + 6.80; 63 KiB | 7.54 + 7.09; 64 KiB |
| 256 MiB | 7.07 + 6.50; 70 KiB | 7.44 + 7.04; 69 KiB | 7.56 + 7.07; 69 KiB |

**read_last_50** (ms: p50 / p95 / p99; 200 samples per cell)

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 0.02 / 0.02 / 0.04 | 0.02 / 0.03 / 0.05 | 0.02 / 0.02 / 0.04 |
| 16 MiB | 0.02 / 0.02 / 0.03 | 0.02 / 0.02 / 0.03 | 0.02 / 0.03 / 0.04 |
| 64 MiB | 0.02 / 0.03 / 0.05 | 0.02 / 0.03 / 0.03 | 0.02 / 0.04 / 0.06 |
| 256 MiB | 0.02 / 0.02 / 0.04 | 0.02 / 0.03 / 0.04 | 0.02 / 0.03 / 0.05 |

**unread_after_p** (ms: p50 / p95 / p99; 200 samples per cell)

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 0.07 / 0.11 / 0.12 | 0.08 / 0.12 / 0.14 | 0.08 / 0.12 / 0.13 |
| 16 MiB | 0.07 / 0.12 / 0.13 | 0.07 / 0.12 / 0.12 | 0.08 / 0.12 / 0.14 |
| 64 MiB | 0.08 / 0.12 / 0.14 | 0.07 / 0.12 / 0.13 | 0.08 / 0.12 / 0.14 |
| 256 MiB | 0.07 / 0.12 / 0.12 | 0.07 / 0.12 / 0.12 | 0.07 / 0.12 / 0.14 |

**edit_recent** (ms: p50 / p95 / p99; 200 samples per cell)

| Size | A | A2 | B | C |
| --- | ---: | ---: | ---: | ---: |
| 1 MiB | 14.0 / 19.6 / 23.4 | 32.6 / 38.1 / 45.6 | 9.60 / 14.3 / 17.1 (22.0% fallback) | 14.1 / 18.9 / 22.8 |
| 16 MiB | 13.8 / 19.6 / 23.1 | 224 / 409 / 690 | 9.95 / 15.8 / 22.6 (22.0% fallback) | 13.7 / 18.1 / 23.9 |
| 64 MiB | 14.1 / 17.9 / 25.0 | 783 / 869 / 982 | 10.3 / 15.1 / 16.0 (19.5% fallback) | 14.0 / 15.8 / 42.3 |
| 256 MiB | 14.1 / 18.7 / 23.1 | 3,337 / 4,040 / 6,087 | 9.76 / 14.0 / 18.0 (17.0% fallback) | 13.8 / 17.6 / 23.4 |

Split of the mean (ledger ms + file ms [+ scrub ms]), bytes written (wchar) per op:

| Size | A | A2 | B | C |
| --- | ---: | ---: | ---: | ---: |
| 1 MiB | 7.83 + 7.26; 119 KiB | 7.67 + 25.5; 1.1 MiB | 6.73 + 3.96; 48 KiB | 7.57 + 7.11; 124 KiB |
| 16 MiB | 7.55 + 6.82; 134 KiB | 8.88 + 238; 16.1 MiB | 7.32 + 4.21; 56 KiB | 7.49 + 6.87; 129 KiB |
| 64 MiB | 7.67 + 7.06; 138 KiB | 8.40 + 773; 64.1 MiB | 7.17 + 4.12; 50 KiB | 7.41 + 6.94; 121 KiB |
| 256 MiB | 7.77 + 7.05; 136 KiB | 9.23 + 3,444; 256.1 MiB | 6.77 + 3.89; 47 KiB | 7.49 + 6.94; 124 KiB |

**edit_random** (ms: p50 / p95 / p99; 200 samples per cell)

| Size | A | A2 | B | C |
| --- | ---: | ---: | ---: | ---: |
| 1 MiB | 19.5 / 29.0 / 33.1 | 36.6 / 50.2 / 57.4 | 9.80 / 20.5 / 23.9 (27.0% fallback) | 19.1 / 28.9 / 33.1 |
| 16 MiB | 162 / 271 / 292 | 297 / 366 / 377 | 10.2 / 190 / 244 (16.5% fallback) | 50.0 / 90.8 / 100 |
| 64 MiB | 589 / 1,058 / 1,116 | 1,114 / 1,370 / 1,470 | 11.7 / 694 / 896 (22.5% fallback) | 47.5 / 94.3 / 103 |
| 256 MiB | 2,015 / 4,053 / 4,244 | 4,495 / 5,930 / 6,676 | 10.6 / 1,819 / 3,107 (17.5% fallback) | 54.1 / 107 / 142 |

Split of the mean (ledger ms + file ms [+ scrub ms]), bytes written (wchar) per op:

| Size | A | A2 | B | C |
| --- | ---: | ---: | ---: | ---: |
| 1 MiB | 11.5 + 8.68; 1.0 MiB | 12.0 + 26.4; 1.5 MiB | 7.67 + 4.55; 279 KiB | 11.3 + 8.24; 1.0 MiB |
| 16 MiB | 85.7 + 71.5; 18.8 MiB | 84.3 + 213; 25.7 MiB | 17.0 + 13.8; 2.6 MiB | 28.9 + 22.1; 4.8 MiB |
| 64 MiB | 308 + 278; 78.5 MiB | 304 + 801; 107.8 MiB | 58.9 + 56.1; 15.2 MiB | 29.2 + 22.1; 5.2 MiB |
| 256 MiB | 1,073 + 982; 301.2 MiB | 1,166 + 3,394; 431.1 MiB | 130 + 133; 36.7 MiB | 32.3 + 26.2; 5.0 MiB |

**redact_recent** (ms: p50 / p95 / p99; 200 samples per cell)

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 26.8 / 31.6 / 35.8 | 23.4 / 28.4 / 30.5 (0.0% fallback) | 27.8 / 32.3 / 38.8 |
| 16 MiB | 28.1 / 34.6 / 39.8 | 23.8 / 28.5 / 34.8 (0.0% fallback) | 27.7 / 32.3 / 39.3 |
| 64 MiB | 33.3 / 44.9 / 76.9 | 23.5 / 26.7 / 34.9 (0.0% fallback) | 27.1 / 33.8 / 47.3 |
| 256 MiB | 27.4 / 36.9 / 51.1 | 23.9 / 31.0 / 56.5 (0.0% fallback) | 27.9 / 33.6 / 45.3 |

Split of the mean (ledger ms + file ms [+ scrub ms]), bytes written (wchar) per op:

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 13.8 + 6.21 + 6.99; 168 KiB | 13.9 + 3.34 + 6.99; 60 KiB | 14.3 + 6.58 + 7.30; 177 KiB |
| 16 MiB | 14.6 + 6.45 + 7.52; 208 KiB | 14.0 + 3.45 + 7.11; 60 KiB | 14.5 + 6.26 + 7.28; 160 KiB |
| 64 MiB | 18.1 + 8.30 + 9.07; 187 KiB | 13.8 + 3.38 + 6.94; 57 KiB | 14.2 + 6.41 + 7.17; 155 KiB |
| 256 MiB | 14.4 + 6.44 + 7.34; 178 KiB | 14.6 + 3.48 + 7.34; 56 KiB | 14.5 + 6.58 + 7.50; 176 KiB |

**redact_random** (ms: p50 / p95 / p99; 200 samples per cell)

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 33.4 / 40.5 / 46.2 | 23.1 / 26.0 / 31.6 (0.0% fallback) | 33.9 / 40.9 / 46.7 |
| 16 MiB | 165 / 287 / 296 | 23.6 / 27.4 / 31.9 (0.0% fallback) | 67.2 / 108 / 115 |
| 64 MiB | 569 / 1,231 / 1,728 | 24.0 / 28.7 / 32.7 (0.0% fallback) | 73.0 / 110 / 126 |
| 256 MiB | 2,087 / 4,208 / 5,174 | 24.0 / 27.6 / 36.6 (0.0% fallback) | 79.0 / 122 / 136 |

Split of the mean (ledger ms + file ms [+ scrub ms]), bytes written (wchar) per op:

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 17.3 + 7.83 + 8.14; 1.2 MiB | 13.5 + 3.29 + 6.80; 54 KiB | 17.5 + 7.92 + 8.28; 1.2 MiB |
| 16 MiB | 83.4 + 70.5 + 9.84; 18.0 MiB | 13.9 + 3.52 + 6.93; 56 KiB | 30.4 + 24.4 + 13.7; 4.5 MiB |
| 64 MiB | 311 + 285 + 4.03; 72.8 MiB | 14.0 + 3.56 + 7.14; 65 KiB | 30.9 + 25.3 + 13.7; 4.9 MiB |
| 256 MiB | 1,158 + 1,076 + 6.71; 310.1 MiB | 14.1 + 3.51 + 7.13; 83 KiB | 33.7 + 30.1 + 15.1; 5.3 MiB |

**reanchor_typo** (ms: p50 / p95 / p99; 20 samples per cell)

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 14.2 / 18.4 / 26.4 | 13.8 / 15.5 / 22.9 | 14.5 / 15.7 / 29.5 |
| 16 MiB | 129 / 141 / 169 | 109 / 119 / 120 | 39.3 / 44.5 / 54.9 |
| 64 MiB | 503 / 548 / 570 | 417 / 443 / 476 | 39.5 / 41.7 / 53.9 |
| 256 MiB | 2,184 / 2,425 / 2,555 | 1,877 / 2,005 / 2,055 | 72.4 / 107 / 179 |

Split of the mean (ledger ms + file ms [+ scrub ms]), bytes written (wchar) per op:

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 15.1 + 0.00; 18 KiB | 14.3 + 0.00; 17 KiB | 15.3 + 0.00; 19 KiB |
| 16 MiB | 131 + 0.00; 17 KiB | 109 + 0.00; 16 KiB | 39.8 + 0.00; 18 KiB |
| 64 MiB | 505 + 0.00; 21 KiB | 421 + 0.00; 22 KiB | 38.5 + 0.00; 25 KiB |
| 256 MiB | 2,247 + 0.00; 31 KiB | 1,867 + 0.00; 30 KiB | 80.1 + 0.00; 34 KiB |

**reanchor_insert** (ms: p50 / p95 / p99; 20 samples per cell)

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 20.8 / 23.1 / 28.3 | 16.1 / 23.9 / 38.6 | 21.4 / 25.6 / 34.4 |
| 16 MiB | 237 / 321 / 331 | 189 / 260 / 374 | 70.6 / 91.3 / 91.5 |
| 64 MiB | 1,046 / 1,176 / 1,240 | 810 / 937 / 947 | 73.0 / 94.2 / 94.5 |
| 256 MiB | 4,343 / 4,798 / 5,097 | 2,716 / 3,846 / 3,965 | 102 / 278 / 449 |

Split of the mean (ledger ms + file ms [+ scrub ms]), bytes written (wchar) per op:

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 20.5 + 0.00; 362 KiB | 17.8 + 0.00; 231 KiB | 21.3 + 0.00; 375 KiB |
| 16 MiB | 234 + 0.00; 7.0 MiB | 198 + 0.00; 5.5 MiB | 64.7 + 0.00; 1.7 MiB |
| 64 MiB | 974 + 0.00; 39.0 MiB | 776 + 0.00; 29.5 MiB | 68.4 + 0.00; 2.0 MiB |
| 256 MiB | 3,981 + 0.00; 140.3 MiB | 2,719 + 0.00; 68.2 MiB | 124 + 0.00; 1.9 MiB |

**reanchor_trim** (ms: p50 / p95 / p99; 1 samples per cell)

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 5.70 / 5.70 / 5.70 | 19.8 / 19.8 / 19.8 | 5.72 / 5.72 / 5.72 |
| 16 MiB | 94.4 / 94.4 / 94.4 | 240 / 240 / 240 | 16.5 / 16.5 / 16.5 |
| 64 MiB | 418 / 418 / 418 | 940 / 940 / 940 | 25.0 / 25.0 / 25.0 |
| 256 MiB | 2,265 / 2,265 / 2,265 | 4,056 / 4,056 / 4,056 | 25.3 / 25.3 / 25.3 |

Split of the mean (ledger ms + file ms [+ scrub ms]), bytes written (wchar) per op:

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 5.70 + 0.00; 0 KiB | 19.8 + 0.00; 431 KiB | 5.72 + 0.00; 0 KiB |
| 16 MiB | 94.4 + 0.00; 0 KiB | 240 + 0.00; 11.3 MiB | 16.5 + 0.00; 0 KiB |
| 64 MiB | 418 + 0.00; 0 KiB | 940 + 0.00; 46.0 MiB | 24.9 + 0.00; 0 KiB |
| 256 MiB | 2,265 + 0.00; 0 KiB | 4,056 + 0.00; 183.5 MiB | 25.3 + 0.00; 0 KiB |

**append_after_foreign_edit** (ms: p50 / p95 / p99; 5 samples per cell)

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 13.4 / 18.6 / 18.6 | 13.8 / 15.4 / 15.4 | 13.4 / 19.3 / 19.3 |
| 16 MiB | 13.2 / 55.6 / 55.6 | 13.9 / 24.2 / 24.2 | 13.0 / 26.7 / 26.7 |
| 64 MiB | 13.0 / 160 / 160 | 13.2 / 55.1 / 55.1 | 14.7 / 34.4 / 34.4 |
| 256 MiB | 14.9 / 23.7 / 23.7 | 13.5 / 24.0 / 24.0 | 13.7 / 25.7 / 25.7 |

Split of the mean (ledger ms + file ms [+ scrub ms]), bytes written (wchar) per op:

| Size | A | B | C |
| --- | ---: | ---: | ---: |
| 1 MiB | 7.06 + 7.28; 46 KiB | 7.07 + 7.21; 35 KiB | 7.13 + 7.26; 45 KiB |
| 16 MiB | 11.8 + 10.0; 48 KiB | 8.75 + 7.01; 45 KiB | 8.04 + 7.67; 41 KiB |
| 64 MiB | 29.1 + 13.7; 60 KiB | 15.0 + 6.51; 50 KiB | 11.4 + 6.93; 55 KiB |
| 256 MiB | 9.19 + 6.99; 56 KiB | 8.86 + 6.66; 50 KiB | 9.06 + 6.83; 48 KiB |

### Flushes and reads, counted

Syscalls per operation (strace, mean of 20 samples, 256 MiB; the 1, 16 and 64 MiB rows are in
`perf/spikes/record-files/results/tables.md`):

| Operation | Layout | Flushes | of which | Reads | Writes | Notes |
| --- | --- | ---: | --- | ---: | ---: | --- |
| append | A, B, C | 2.08–2.12 | 1 fdatasync chat, 1.08 fsync ledger WAL | 5.3–5.6 | 32 | 1 `pwrite` to the chat file, about 29 WAL page writes, 2 index WAL page writes (not flushed), 2 `stat` (foreign-edit check, own write) |
| append | A, B, C at 1 MiB | 2.08 | same | 0 | 21 | reads grow as the ledger outgrows SQLite's 2 MiB page cache |
| read last 50 | all | 0 | — | 1 | 0 | 1 `stat` + 1 ranged `pread` |
| unread after P | all | 0 | — | 3.1 | 0 | |
| edit recent | A, C | 2.00 | 1 fdatasync chat, 1 fsync WAL | 2.3 | 26 | the tail after the record is under 1 MiB |
| edit recent | B | 2.00 | same | 1.6 | 18 | in place 83% of the time |
| edit random | A | 151 | about 120 `fdatasync` of the chat (1 per MiB of tail, `fsio::paced`) + WAL | 36,695 | 81,189 | the offset shift spills to a SQLite temporary file |
| edit random | A2 | 263 | 257 `fdatasync` (1 per MiB of the whole file) + file `fsync` + folder `fsync` + WAL | 31,561 | 70,240 | |
| edit random | B | 30 | 2 when it fits; a fallback as A | 6,555 | 13,849 | 17.5% fallbacks |
| edit random | C | 4.5 | 1 per MiB of the segment's tail + WAL | 432 | 1,081 | |
| redact (any) | B | 5.00 | 1 chat, 1 WAL commit, 3 for `wal_checkpoint(TRUNCATE)` | 16 | 38 | the scrub's checkpoint is the extra 3 flushes |
| redact recent | A, C | 5.00 | same | 17–44 | 48–75 | |
| redact random | A | 125 | as edit random + 3 | 43,037 | 82,152 | |
| foreign typo, then check | A | 1.5 | 1 commit (+ checkpoint) | 29,049 | 20 | reads the whole 256 MiB file and the file's ledger rows |
| foreign typo, then check | C | 1.5 | same | 510 | 22 | reads only the edited segment |

### History after 1,000 edits

1,000 recent edits (the same mix) add **240–280 KiB to the ledger** at every
size and in every layout: 1,000 version rows of 160–185 KiB of bodies, plus
row and index overhead. So history costs about 250 bytes per edit, independent
of the file's length. The chat file grows by the edits' net length (15–30 KB).
B's slack does not grow, because a record that outgrows its slot gets a fresh
one. History has no bound in this spike; rule 7 needs one (section 5).

### Correctness after every case

Each case ended by comparing every file byte for byte with what the ledger
renders, after all its edits, redactions, foreign edits and the appends that
followed them. All 16 timed cases and all 16 traced cases passed.

## 4. Readability, hand edits and foreign editors

| | A (one file, tail rewrite) | B (one file, slack) | C (monthly segments) |
| --- | --- | --- | --- |
| Reads as plain Markdown | yes | yes. Each record ends with an invisible line of spaces (25% of the record, at least 16). It renders exactly as a blank line, but an editor that shows whitespace shows it, and git diffs show it | yes, but one chat is many files; reading or searching it in an editor means opening a folder |
| Hand edit of one record | detected by `stat`; the whole file is re-read and re-anchored: 2.2 s at 256 MiB, 0.5 s at 64 MiB, 0.13 s at 16 MiB | same (1.9 s at 256 MiB) | only that segment is re-read: 40–100 ms |
| Editor set to trim trailing whitespace (VS Code `files.trimTrailingWhitespace`, `.editorconfig` `trim_trailing_whitespace`) | no effect | removes all slack in one save. Every record moves, so every ledger row is rewritten: 4.1 s and 184 MiB of SQLite writes at 256 MiB. Edits then fall back to tail rewrites until records get new slack | no effect |
| Inserted text that shifts later records | the offsets of all later records are rewritten: 4.3 s at 256 MiB | 2.7 s | the rest of one segment: about 100 ms |

What re-anchoring does, and what the spike showed it cannot do yet:

- **Detection** is one `stat` per operation, comparing size, mtime (ns) and
  inode with the host's own last write. Editors that save by writing a
  temporary file and renaming it (vim, most GUI editors) always change the
  inode. One case slips through: a same-size, in-place edit within one
  timestamp tick of the host's write. That needs a content check, which costs
  a full read.
- **Cost** is a full read, a header scan, a hash of every body, and loading the
  file's ledger rows. It is proportional to the file even when one letter
  changed: a save that changed nothing (A, trim) still costs 2.3 s at 256 MiB.
  It could probably be made several times cheaper (load only offsets and
  hashes), but it stays proportional to the file.
- **A changed body** becomes a new version stamped `external`. That is right
  for the ledger, but the ledger cannot tell which person made the edit.
- **A mangled anchor** (seen on dev-01 while building the spike, when the
  simulated typo hit an anchor) turns the record into "gone" plus a new
  `external` record. The record loses its stamp, and replies that link to the
  old anchor break.
- **A changed name in a header** (`**Lee**` edited to `**Sam**`) is not
  noticed: the body hash is unchanged, so the file now shows a name the ledger
  does not hold. The host should compare the header with the stamp and either
  re-render it or flag it.
- **Recovery normalises.** After a crash, recovery re-renders the region of
  the last host write in host form. A foreign formatting change inside that
  region (one write's worth) is replaced.

## 5. What was not measured

- **Power loss.** The crash tests abort the process, which keeps the page
  cache. They prove the write order and recovery logic, not the disk's
  behaviour when the power fails.
- **ZFS** (production) and the HDD root. ext4 on SSD only.
- **Cold cache.** All reads hit the page cache. A cold `read_last_50` would add
  one random read; a cold re-anchor would read the whole file from disk.
- **Concurrency and group commit.** One writer thread and one chat. Concurrent
  appends to one space, or several chats sharing a space's ledger, were not
  measured; the per-space writer's group commit should amortise the WAL fsync.
- **Index work beyond one row per record**: full-text search, links, backlinks.
  A redaction would also have to scrub any full-text index, and that cost was
  not measured.
- **Scrub beyond SQLite.** `secure_delete` plus a truncating checkpoint removes
  the words from `ledger.db` and its WAL as files. It does not remove them from
  free blocks on the disk, filesystem snapshots (ZFS), backups already taken,
  or (in A2) the replaced file's old blocks. B and A overwrite the record in
  place, but on a copy-on-write filesystem that writes new blocks anyway.
- **History retention.** No pruning; the ledger keeps every version and a copy
  of every current body (section 3).
- **Editors' own cost** of opening and saving a 256 MiB Markdown file.
- **Very long single records** and attachments.
- **Slack tuning.** B used 25% (at least 16 bytes) only. The fallback rate was
  17–27% with this edit mix, almost all from edits that grow by 30 bytes or
  more.

## 6. Recommendation

**1. Do segments stay? No: drop monthly segments from the host (D5 / Q20 cut),
with two cheap rules instead.**

- Append, reads, unread counts, and edits or redactions of recent records cost
  the same at 1 MiB and at 256 MiB. Segments buy nothing for them.
- **Write every shrinking change in place, padded with spaces to the old
  length** (B's trick, used only when it is needed). A redaction always
  shrinks, so a redaction anywhere in the file costs a flat 24 ms. B measures
  exactly this write pattern (one record-sized write and one flush); the
  "A plus padding on shrink" combination itself was not run as a separate
  layout. Readability cost: one invisible line of spaces, only in records that
  were redacted or shortened. Slack on every record (full B) is not worth it:
  the file is 25% bigger, trimming editors erase the slack, and 17–27% of
  edits overflow their slot anyway.
- A **growing edit of an old record** rewrites the tail: about 16 ms per MiB
  after the record (2 s p50 and 4 s p99 at 256 MiB, 0.6 s at 64 MiB). Chat's
  48-hour edit window means Chat never takes this path; its edits are recent.
  A record kind that allows growing edits anywhere should declare a size cap
  for its files (64 MiB keeps the worst case under about 1 s). That cap belongs
  to the kind, not to a host segment mechanism.
- **Store offsets so that a shift is not O(rows)**, or accept it: half of the
  old-edit cost is `UPDATE … SET off = off + d` over every later record. One way
  out is to keep only `(seq, len)` and derive offsets in the read path, or to
  rebuild offsets lazily on read.
- **Foreign edits of huge files** cost a full re-read (2.2 s at 256 MiB). That
  is the strongest argument for segments, and it is weak: a 256 MiB chat is five
  years of a 600-message-a-day chat, and the editor opening it will take longer
  than the host does.

**2. Is "2 indexed reads and 2 flushes, whatever the file's length" true?**
**Yes for append**, measured: 2 SQL lookups, and 1 WAL fsync plus 1 file
fdatasync, which costs 13 ms p50 at every size on this disk. Three corrections
for the design text:

- A checkpoint adds a flush about once every 12 appends (2.08–2.12 average).
- Bytes written per append are not constant: 36–70 KiB of SQLite WAL pages,
  rising slowly with the ledger's B-tree depth. The 200-byte message is the
  smallest part.
- The design's table says edit and redact cost "1 file rewrite (cost grows with
  the file)". That is wrong for recent records and for any shrinking change:
  both are 2 flushes and flat (redaction 5 flushes with the WAL scrub). It
  holds only for growing edits of old records, and for whole-file replacement
  (A2: 3.3 s at 256 MiB even for a recent record), which the host should never
  use for records.

**3. Two things the design must add, whatever the layout:**

- **The ledger doubles storage.** "Recovery writes the file from the ledger"
  means the ledger holds every current body: 404 MiB of ledger for a 256 MiB
  chat. Either accept it and state the bound (rule 7), or keep bodies only for
  a recovery window (until the file write is confirmed and a checkpoint passes)
  plus true history, and recover older bytes from the file itself.
- **Redaction scrub needs a WAL truncate** (3 extra flushes, about 7 ms) and
  `secure_delete`. Without them, the old words stay in `ledger.db-wal` and in
  free pages. The spike checked: with them, no file in the chat's folder
  contains the words afterwards. Snapshots, backups and free disk blocks are
  outside this guarantee, and the design should say so.

## Simplification opportunities

- Segments, the `segment_full` error, segment rollover and the "chat vs this
  month's file" problem can all go (above). The host keeps one record file per
  chat.
- Offsets do not need to be stored with each ledger row. `(seq, len)` plus a
  prefix sum (or a lazily rebuilt offset index) removes half of the old-edit
  cost and the O(rows) `UPDATE`.
- The ledger's copy of every current body exists only for crash recovery. A
  bounded recovery window removes most of the ledger's size.
- `heads.pending_from` (one column) is the whole crash protocol for appends,
  edits and redactions. The design's "expected file length" rule for appends
  is a special case of it and can be stated once for all record writes.
- Re-anchoring is one function for all foreign edits. It does not need
  per-editor special cases; it needs the header-against-stamp check noted in
  section 4.
- `fsio::paced::write_all` takes `at` relative to the start of the buffer and
  uses it for `fadvise`. That is correct for its current callers, which write
  from offset 0, but a caller writing from a file offset (as this spike's tail
  rewrite does) drops the wrong cache range. Pass the file offset if record
  writes ever use it.
