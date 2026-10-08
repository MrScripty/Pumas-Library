/* Controlled Linux process fixture. No Pumas API, ORT, model or inference claim. */
#include <arpa/inet.h>
#include <errno.h>
#include <netinet/in.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#ifndef FIXTURE_RESPONSE
#error Supply the controlled test handshake, never a production build identity.
#endif

static volatile sig_atomic_t stopping = 0;
static void stop(int ignored) { (void)ignored; stopping = 1; }

int main(int argc, char **argv) {
    if (argc != 5 || strcmp(argv[1], "--launcher-root") ||
        strcmp(argv[3], "--port") || strcmp(argv[4], "0")) return 2;
    if (getenv("LD_AUDIT") || getenv("DYLD_INSERT_LIBRARIES") ||
        getenv("ORT_DYLIB_PATH") || getenv("PUMAS_FIXTURE_AMBIENT")) return 7;
    struct sigaction action;
    memset(&action, 0, sizeof(action));
    action.sa_handler = stop;
    sigaction(SIGINT, &action, NULL);
    sigaction(SIGTERM, &action, NULL);
    signal(SIGPIPE, SIG_IGN);
    int listener = socket(AF_INET, SOCK_STREAM, 0);
    struct sockaddr_in address;
    memset(&address, 0, sizeof(address));
    address.sin_family = AF_INET;
    address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    if (listener < 0 || bind(listener, (struct sockaddr *)&address, sizeof(address)) ||
        listen(listener, 8)) return 3;
    socklen_t size = sizeof(address);
    if (getsockname(listener, (struct sockaddr *)&address, &size)) return 4;
    printf("RPC_PORT=%u\n", ntohs(address.sin_port));
    fflush(stdout);
    while (!stopping) {
        int client = accept(listener, NULL, NULL);
        if (client < 0) { if (errno == EINTR) continue; return 5; }
        char request[1024] = {0};
        ssize_t received = recv(client, request, sizeof(request) - 1, 0);
        const char *body = FIXTURE_RESPONSE;
        int found = received > 0 && !strncmp(request, "GET /.well-known/pumas ", 23);
        char header[256];
        int length = snprintf(header, sizeof(header),
            "HTTP/1.1 %s\r\nContent-Type: application/json\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n",
            found ? "200 OK" : "404 Not Found", found ? strlen(body) : (size_t)2);
        if (send(client, header, (size_t)length, 0) < 0 ||
            send(client, found ? body : "{}", found ? strlen(body) : 2, 0) < 0) {
            close(client); close(listener); return 6;
        }
        close(client);
    }
    close(listener);
    return 0;
}
