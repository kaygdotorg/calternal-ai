#!/bin/sh
# Run on the dev VM under its gate lock. This counts syscalls, not time.
# Pass the integration-test executable printed by cargo test --test chat --no-run.
set -eu
trace=$(mktemp)
trap 'rm -f "$trace"' EXIT HUP INT TERM
strace -f -qq -yy -e trace=write,pwrite64,read,pread64,fsync,fdatasync,syncfs \
    -o "$trace" "$1" --exact thousand_appends --nocapture
awk '
    /HOST_APPEND_BEGIN/ { in_append=1; next }
    /HOST_APPEND_END/ { in_append=0; ended=1 }
    !in_append { next }
    /fsync\(|fdatasync\(|syncfs\(/ && /chat\.md/ { item_flush++ }
    /fsync\(|fdatasync\(|syncfs\(/ && /ledger\.sqlite3/ { ledger_flush++ }
    /fsync\(|fdatasync\(|syncfs\(/ && !/chat\.md|ledger\.sqlite3/ { other_flush++ }
    /(^|[[:space:]])(read|pread64)\(/ && /chat\.md/ { item_read++ }
    /(^|[[:space:]])(read|pread64)\(/ && /ledger\.sqlite3/ { ledger_read++ }
    /(^|[[:space:]])(read|pread64)\(/ && /auth\.db/ { authority_read++ }
    END {
        if (!ended || item_flush != 1000) exit 1
        printf "1000 appends: %d item reads, %d ledger reads, %d authority reads, %d item flushes, %d ledger flushes, %d other flushes\n",
            item_read, ledger_read, authority_read, item_flush, ledger_flush, other_flush
    }
' "$trace"
