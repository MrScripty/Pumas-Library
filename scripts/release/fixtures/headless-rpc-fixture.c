/* Controlled Linux process fixture. No Pumas authentication, registry or inference proof. */
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
#error Supply a controlled HttpServiceDescription, never a production build identity.
#endif
#ifndef FIXTURE_MODE
#define FIXTURE_MODE 0
#endif

static volatile sig_atomic_t stopping = 0;
static void stop(int ignored) { (void)ignored; stopping = 1; }

static void json_string(FILE *output, const char *value) {
    fputc('"', output);
    for (; *value; value++) {
        if (*value == '"' || *value == '\\') fputc('\\', output);
        if ((unsigned char)*value < 32) fprintf(output, "\\u%04x", (unsigned char)*value);
        else fputc(*value, output);
    }
    fputc('"', output);
}

int main(int argc, char **argv) {
    int observing = argc == 4 && !strcmp(argv[1], "--describe-local-http") &&
        !strcmp(argv[2], "--launcher-root");
    const char *root = observing ? argv[3] : (argc == 5 ? argv[2] : "");
    char observation[4096];
    if (snprintf(observation, sizeof(observation), "%s/fixture-observation.json", root) >=
        (int)sizeof(observation)) return 2;
    if (observing) {
        if (FIXTURE_MODE == 6) { for (;;) pause(); }
        /* A controlled stand-in for the real CLI's authenticated observation. */
        FILE *input = fopen(observation, "rb");
        if (!input) return 8;
        int c;
        while ((c = fgetc(input)) != EOF) fputc(c, stdout);
        fclose(input);
        return 0;
    }
    if (argc != 5 || strcmp(argv[1], "--launcher-root") ||
        strcmp(argv[3], "--port") || strcmp(argv[4], "0")) return 2;
    if (getenv("LD_AUDIT") || getenv("DYLD_INSERT_LIBRARIES") ||
        getenv("ORT_DYLIB_PATH") || getenv("PUMAS_FIXTURE_AMBIENT")) return 7;
    if (FIXTURE_MODE == 1) return 9;
    struct sigaction action;
    memset(&action, 0, sizeof(action));
    action.sa_handler = FIXTURE_MODE == 5 ? SIG_IGN : stop;
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
    char endpoint[128];
    snprintf(endpoint, sizeof(endpoint), "http://127.0.0.1:%u", ntohs(address.sin_port));
    char *body = NULL;
    size_t body_length = 0;
    FILE *json = open_memstream(&body, &body_length);
    if (!json) return 10;
    const char *p = FIXTURE_RESPONSE;
    while (*p) {
        if (!strncmp(p, "\"__ROOT__\"", 10)) { json_string(json, root); p += 10; }
        else if (!strncmp(p, "\"__ENDPOINT__\"", 14)) { json_string(json, endpoint); p += 14; }
        else fputc(*p++, json);
    }
    fclose(json);
    FILE *record = fopen(observation, "wb");
    if (!record) { free(body); return 11; }
    fwrite(body, 1, body_length, record);
    fclose(record);
    const char *registry = getenv("PUMAS_REGISTRY_DB_PATH");
    if (registry) { FILE *db = fopen(registry, "ab"); if (db) fclose(db); }
    if (FIXTURE_MODE != 2) {
        printf("RPC_PORT=%u\n", FIXTURE_MODE == 3 ? 0 : ntohs(address.sin_port));
        fflush(stdout);
    }
    while (!stopping) {
        int client = accept(listener, NULL, NULL);
        if (client < 0) { if (errno == EINTR) continue; free(body); return 5; }
        char request[1024] = {0};
        ssize_t received = recv(client, request, sizeof(request) - 1, 0);
        const char *route = "GET /.well-known/pumas ";
        int found = received > 0 && !strncmp(request, route, strlen(route));
        char header[512];
        int length = snprintf(header, sizeof(header),
            "HTTP/1.1 %s\r\nContent-Type: application/json\r\nContent-Length: %zu\r\n"
            "Location: http://127.0.0.1:1/elsewhere\r\nConnection: close\r\n\r\n",
            FIXTURE_MODE == 4 ? "302 Found" : (found ? "200 OK" : "404 Not Found"),
            found ? body_length : (size_t)2);
        if (send(client, header, (size_t)length, 0) < 0 ||
            send(client, found ? body : "{}", found ? body_length : 2, 0) < 0) {
            close(client); close(listener); free(body); return 6;
        }
        close(client);
    }
    close(listener);
    free(body);
    unlink(observation);
    return 0;
}
