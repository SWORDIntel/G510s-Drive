/* SPDX-License-Identifier: GPL-2.0-or-later
 * g15daemon LCD client: one unsigned byte per pixel, 160x43 (6880 bytes).
 * Uses the distro's existing g15daemon-client ABI; never talks to USB directly.
 */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <g15daemon_client.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

enum { LCD_WIDTH = 160, LCD_HEIGHT = 43, FRAME_BYTES = LCD_WIDTH * LCD_HEIGHT };
static volatile sig_atomic_t stopping = 0;

static void on_signal(int signo) { (void)signo; stopping = 1; }

static void usage(const char *program) {
    fprintf(stderr, "Usage: %s [--file RAW] [--hold]\n"
                    "Read 160x43 byte-per-pixel frames (6880 bytes each) from stdin or RAW.\n"
                    "--hold keeps the screen registered after EOF until SIGINT/SIGTERM.\n",
                    program);
}

/* 1=complete frame, 0=clean EOF, -1=truncated frame/read failure */
static int read_frame(FILE *input, uint8_t *frame) {
    size_t offset = 0;
    while (offset < FRAME_BYTES && !stopping) {
        size_t n = fread(frame + offset, 1, FRAME_BYTES - offset, input);
        if (n) { offset += n; continue; }
        if (ferror(input) && errno == EINTR) {
            clearerr(input);
            continue;
        }
        if (ferror(input)) return -1;
        if (feof(input)) return offset == 0 ? 0 : -1;
    }
    return stopping ? 0 : 1;
}

int main(int argc, char **argv) {
    const char *file = NULL;
    int hold = 0;
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--hold") == 0) hold = 1;
        else if (strcmp(argv[i], "--file") == 0 && i + 1 < argc) file = argv[++i];
        else if (strcmp(argv[i], "--help") == 0 || strcmp(argv[i], "-h") == 0) {
            usage(argv[0]); return 0;
        } else { usage(argv[0]); return 2; }
    }
    FILE *in = file ? fopen(file, "rb") : stdin;
    if (!in) { perror("frame input"); return 2; }
    signal(SIGINT, on_signal);
    signal(SIGTERM, on_signal);

    int screen = new_g15_screen(G15_PIXELBUF);
    if (screen < 0) {
        fputs("Cannot connect to g15daemon on localhost:15550. Check its service and USB access.\n", stderr);
        if (file) fclose(in);
        return 3;
    }
    uint8_t frame[FRAME_BYTES];
    unsigned long sent = 0;
    int result = 0;
    while (!stopping) {
        int n = read_frame(in, frame);
        if (n == 0) break;
        if (n < 0) {
            fputs("Input ended mid-frame or could not be read (expected 6880 bytes).\n", stderr);
            result = 4; break;
        }
        if (g15_send(screen, (char *)frame, FRAME_BYTES) != 0) {
            fputs("g15daemon frame send failed.\n", stderr);
            result = 5; break;
        }
        sent++;
    }
    if (file) fclose(in);
    if (hold && sent && !result) while (!stopping) pause();
    g15_close_screen(screen);
    return result;
}
