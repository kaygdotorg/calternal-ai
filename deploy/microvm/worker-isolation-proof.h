// Only included in the KVM test image. These checks run after the same
// namespace, mount, UID and seccomp setup as the ordinary package exec.
#ifndef _GNU_SOURCE
#define _GNU_SOURCE 1
#endif
#include <errno.h>
#include <fcntl.h>
#include <sched.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/ptrace.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <unistd.h>

static void proof(int condition, const char *step) {
    if (condition) return;
    fprintf(stderr, "worker isolation proof: %s (errno %d)\n", step, errno);
    _exit(125);
}

static void run_worker_proof(void) {
    static const char marker[] = "// calternal native launcher proof\n";
    char start[sizeof(marker) - 1];
    int package = open("/plugin/worker.js", O_RDONLY | O_CLOEXEC);
    if (package < 0) return;
    ssize_t marker_bytes = read(package, start, sizeof(start));
    close(package);
    if (marker_bytes != (ssize_t)sizeof(start) || memcmp(start, marker, sizeof(start))) return;

    proof(socket(AF_VSOCK, SOCK_STREAM, 0) == -1 && errno == EPERM, "vsock denied");
    for (const char **path = (const char *[]){
            "/run/agent", "/dev/vdc", "/proc/sys", "/proc/timer_list", NULL}; *path; path++)
        proof(open(*path, O_RDONLY | O_CLOEXEC) == -1, *path);
    FILE *mounts = fopen("/proc/self/mountinfo", "r");
    proof(mounts != NULL, "read worker mounts");
    char line[8192], target[128];
    int mount_count = 0;
    while (fgets(line, sizeof(line), mounts)) {
        proof(sscanf(line, "%*s %*s %*s %*s %127s", target) == 1, "parse mount target");
        proof(!strcmp(target, "/") || !strcmp(target, "/proc") ||
              !strcmp(target, "/dev") || !strcmp(target, "/plugin"), "guest root is detached");
        mount_count++;
    }
    fclose(mounts);
    proof(mount_count == 4, "four private mounts");
    proof(kill(-1, SIGKILL) == -1 && errno == ESRCH, "no process visible outside PID namespace");
    proof(unshare(CLONE_NEWUSER) == -1 && errno == EPERM, "user namespace denied");
    proof(ptrace(PTRACE_ATTACH, 1, NULL, NULL) == -1 && errno == EPERM, "ptrace denied");
    proof(setuid(0) == -1 && errno == EPERM, "setuid denied");
    proof(open("/tmp/calternal-test", O_CREAT | O_EXCL | O_WRONLY, 0600) == -1,
          "write beyond package denied");

    // The trusted fixture bounds the attempt and releases all children even
    // if it hits an unexpected limit. PID 1 and its launcher count too.
    int latch[2], count = 0, limit = 0;
    proof(pipe(latch) == 0, "create child latch");
    while (count < 64) {
        pid_t child = fork();
        if (child < 0) {
            limit = errno;
            break;
        }
        if (!child) {
            char unused;
            close(latch[1]);
            _exit(read(latch[0], &unused, 1) != 0);
        }
        count++;
    }
    close(latch[1]);
    close(latch[0]);
    while (waitpid(-1, NULL, 0) > 0 || errno == EINTR) {}
    proof(count == 62 && limit == EAGAIN, "pids.max rejected the 65th task");

    uint8_t random[32];
    int entropy = open("/dev/urandom", O_RDONLY | O_CLOEXEC);
    proof(entropy >= 0 && read(entropy, random, sizeof(random)) == (ssize_t)sizeof(random),
          "draw kernel bytes");
    close(entropy);
    printf("{\"kernel\":\"");
    for (size_t i = 0; i < sizeof(random); i++) printf("%02x", random[i]);
    puts("\"}");
    fflush(stdout);
}
