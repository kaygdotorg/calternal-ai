#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <grp.h>
#include <linux/filter.h>
#include <linux/seccomp.h>
#include <sched.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mount.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <sys/wait.h>
#include <unistd.h>

// Only the trusted agent UID can invoke this setuid helper. All setup
// happens inside the guest and completes before any package code runs.
static const char root[] = "/opt/standard";

static void require(int result, const char *step) {
    if (result < 0) {
        perror(step);
        _exit(126);
    }
}

static int valid_id(const char *id) {
    if (strlen(id) != 36) return 0;
    for (int i = 0; i < 36; i++) {
        if (i == 8 || i == 13 || i == 18 || i == 23) {
            if (id[i] != '-') return 0;
        } else if (!strchr("0123456789abcdef", id[i])) return 0;
    }
    return 1;
}

static void place(const char *id) {
    char path[128];
    snprintf(path, sizeof(path), "/sys/fs/cgroup/agent/ctr/%s/cgroup.procs", id);
    int fd = open(path, O_WRONLY | O_CLOEXEC | O_NOFOLLOW);
    require(fd, "open cgroup.procs");
    require(write(fd, "0", 1) == 1 ? 0 : -1, "join cgroup");
    require(close(fd), "close cgroup");
}

static size_t read_filter(struct sock_filter *filter, size_t bytes) {
    int fd = open("/opt/calternal-worker.bpf", O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
    require(fd, "open filter");
    size_t read_bytes = 0;
    while (read_bytes < bytes) {
        ssize_t count = read(fd, (char *)filter + read_bytes, bytes - read_bytes);
        require((int)count, "read filter");
        if (!count) break;
        read_bytes += (size_t)count;
    }
    require(close(fd), "close filter");
    if (!read_bytes || read_bytes == bytes || read_bytes % sizeof(*filter)) _exit(126);
    return read_bytes / sizeof(*filter);
}

static void worker(const char *source, const struct sock_fprog *policy, int standby) {
    require(mount(root, root, NULL, MS_BIND, NULL), "bind runtime root");
    require(mount(source, "/opt/standard/plugin", NULL, MS_BIND, NULL), "bind worker code");
    require(mount(NULL, "/opt/standard/plugin", NULL,
                  MS_BIND | MS_REMOUNT | MS_RDONLY | MS_NOSUID | MS_NODEV | MS_NOEXEC,
                  NULL), "seal worker code");
    require(mount("proc", "/opt/standard/proc", "proc",
                  MS_RDONLY | MS_NOSUID | MS_NODEV | MS_NOEXEC, NULL), "mount private proc");
    require(mount("tmpfs", "/opt/standard/dev", "tmpfs", MS_NOSUID | MS_NOEXEC,
                  "mode=0755,size=64k"), "mount private dev");
    const struct { const char *path; int minor; } devices[] = {
        {"/opt/standard/dev/null", 3}, {"/opt/standard/dev/zero", 5},
        {"/opt/standard/dev/full", 7}, {"/opt/standard/dev/random", 8},
        {"/opt/standard/dev/urandom", 9},
    };
    umask(0);
    for (size_t i = 0; i < sizeof(devices)/sizeof(devices[0]); i++)
        require(mknod(devices[i].path, S_IFCHR | 0666, makedev(1, devices[i].minor)), "make device");
    require(symlink("/proc/self/fd", "/opt/standard/dev/fd"), "link private fd");
    require(mount(NULL, "/opt/standard/dev", "tmpfs",
                  MS_REMOUNT | MS_RDONLY | MS_NOSUID | MS_NOEXEC, NULL), "seal devices");
    require(mount(NULL, root, NULL, MS_BIND | MS_REMOUNT | MS_RDONLY | MS_NOSUID | MS_NODEV,
                  NULL), "seal runtime root");
    require(chroot(root), "chroot");
    require(chdir("/plugin"), "enter readonly package");
    require(setgroups(0, NULL), "drop supplementary groups");
    for (int cap = 0; cap <= 40; cap++)
        require(prctl(PR_CAPBSET_DROP, cap, 0, 0, 0), "drop bounding capability");
    require(setresgid(1100, 1100, 1100), "drop gid");
    require(setresuid(1100, 1100, 1100), "drop uid");
    require(prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), "no new privileges");
    require(syscall(SYS_close_range, 3U, ~0U, 0U), "close inherited descriptors");
    require(syscall(SYS_seccomp, SECCOMP_SET_MODE_FILTER, 0, policy), "install worker seccomp");
    if (getuid() != 1100 || getgid() != 1100 || getpid() != 1 ||
        prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) != 1) _exit(126);
    errno = 0;
    if (syscall(SYS_socket, 2, 1, 0) != -1 || errno != EPERM) _exit(126);
    struct statvfs code;
    require(statvfs("/plugin", &code), "check package mount");
    if (!(code.f_flag & ST_RDONLY)) _exit(126);
    char *const command[] = {
        "node", "--permission", "--allow-fs-read=.", "--disallow-code-generation-from-strings",
        "--max-old-space-size=128", "worker.js", NULL,
    };
    char *const parked[] = {
        "/usr/local/bin/node", "--permission", "--allow-addons",
        "--allow-fs-read=/opt/calternal-standby", "--allow-fs-read=/plugin",
        "--disallow-code-generation-from-strings", "--max-old-space-size=128",
        "/opt/calternal-standby/standby.cjs", NULL,
    };
    char *const env[] = {"LANG=C.UTF-8", NULL};
    execve("/usr/local/bin/node", standby ? parked : command, env);
    perror("exec node");
    _exit(126);
}

int main(int argc, char **argv) {
    if ((argc != 2 && (argc != 3 || strcmp(argv[2], "--standby") != 0)) ||
        !valid_id(argv[1]) || getuid() != 1000 || geteuid() != 0) return 126;
    struct sock_filter filter[4097];
    struct sock_fprog policy = { .filter = filter, .len = read_filter(filter, sizeof(filter)) };
    char source[128];
    snprintf(source, sizeof(source), "/run/agent/plain/%s", argv[1]);
    place(argv[1]);
    require(unshare(CLONE_NEWNS | CLONE_NEWPID | CLONE_NEWNET | CLONE_NEWIPC | CLONE_NEWUTS),
            "unshare worker namespaces");
    require(mount(NULL, "/", NULL, MS_REC | MS_PRIVATE, NULL), "private mount propagation");
    pid_t child = fork();
    require(child, "spawn worker in private pid namespace");
    if (!child) worker(source, &policy, argc == 3);
    int status;
    while (waitpid(child, &status, 0) < 0) {
        if (errno != EINTR) return 126;
    }
    return WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status);
}
