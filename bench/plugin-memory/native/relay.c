#include <errno.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <linux/vm_sockets.h>
#include <sys/un.h>
#include <unistd.h>
#include <signal.h>

static int forward(int from, int to) {
    char buf[65536];
    ssize_t count = read(from, buf, sizeof(buf));
    if (count <= 0) return -1;
    for (ssize_t pos = 0; pos < count;) {
        ssize_t n = write(to, buf + pos, count - pos);
        if (n <= 0) return -1;
        pos += n;
    }
    return 0;
}

int main(void) {
    signal(SIGPIPE, SIG_IGN);
    int listener = socket(AF_VSOCK, SOCK_STREAM, 0);
    struct sockaddr_vm vm = {.svm_family = AF_VSOCK, .svm_port = 5000, .svm_cid = VMADDR_CID_ANY};
    if (listener < 0 || bind(listener, (void*)&vm, sizeof(vm)) || listen(listener, 16)) {
        perror("vsock listen"); return 1;
    }
    for (;;) {
        int peer = accept(listener, NULL, NULL);
        if (peer < 0) { if (errno == EINTR) continue; perror("accept"); return 1; }
        int app = socket(AF_UNIX, SOCK_STREAM, 0);
        struct sockaddr_un addr = {.sun_family = AF_UNIX, .sun_path = "/run/runtime.sock"};
        if (app >= 0 && !connect(app, (void*)&addr, sizeof(addr))) {
            struct pollfd p[] = {{peer, POLLIN, 0}, {app, POLLIN, 0}};
            while (poll(p, 2, -1) > 0)
                if ((p[0].revents && forward(peer, app)) || (p[1].revents && forward(app, peer)))
                    break;
        }
        if (app >= 0) close(app);
        close(peer);
    }
}
