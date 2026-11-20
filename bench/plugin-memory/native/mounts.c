#include <errno.h>
#include <stdio.h>
#include <sys/mount.h>

int main(void) {
    if (mount("proc", "/proc", "proc", 0, NULL)) { perror("proc"); return 1; }
    if (mount("sysfs", "/sys", "sysfs", 0, NULL)) { perror("sysfs"); return 1; }
    if (mount("devtmpfs", "/dev", "devtmpfs", 0, "mode=0755") && errno != EBUSY) { perror("devtmpfs"); return 1; }
    if (mount("tmpfs", "/run", "tmpfs", 0, "mode=0755")) { perror("tmpfs"); return 1; }
    return 0;
}
