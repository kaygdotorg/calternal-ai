/* Counts SQLite's actual sync calls in the disposable Linux layout fixture. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdatomic.h>
#include <stdint.h>
#include <unistd.h>

static _Atomic uint64_t calls;

uint64_t step0_sync_calls(void) {
    return atomic_load_explicit(&calls, memory_order_relaxed);
}

int fdatasync(int fd) {
    int (*real)(int) = dlsym(RTLD_NEXT, "fdatasync");
    atomic_fetch_add_explicit(&calls, 1, memory_order_relaxed);
    return real(fd);
}

int fsync(int fd) {
    int (*real)(int) = dlsym(RTLD_NEXT, "fsync");
    atomic_fetch_add_explicit(&calls, 1, memory_order_relaxed);
    return real(fd);
}
