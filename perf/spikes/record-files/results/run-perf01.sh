#!/usr/bin/env bash
# Record-files spike (#107): timings, then syscall counts under strace. Runs under perf-lock.sh.
set -u
cd ~/calternal-ai-perf/spike-records
W=$PWD/work; mkdir -p $W
date -u +"start %FT%TZ" > progress.log
./rfs bench --dir $W/t --sizes 1,16,64,256 --layouts A,A2,B,C --samples 200 --reanchor-samples 20 --history 1000 --raw raw.tsv > timings.txt 2> timings.err
date -u +"timings done %FT%TZ" >> progress.log
T=~/calternal-ai-perf/tools/extracted
LD_LIBRARY_PATH=$T/usr/lib/x86_64-linux-gnu $T/usr/bin/strace -f -y -qq -e trace=read,pread64,write,pwrite64,fsync,fdatasync,sync_file_range,openat,newfstatat,statx,fstat,rename,renameat,renameat2,ftruncate,fadvise64 -o strace.log \
  ./rfs bench --dir $W/s --sizes 1,16,64,256 --layouts A,A2,B,C --samples 20 --reanchor-samples 2 --history 0 --markers --raw raw-strace.tsv > strace-run.txt 2> strace-run.err
python3 count.py strace.log > counts.txt 2> counts.err
rm -f strace.log
date -u +"done %FT%TZ" >> progress.log
