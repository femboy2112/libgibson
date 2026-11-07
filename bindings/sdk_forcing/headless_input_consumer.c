/*
 * LibGibson installed-SDK consumer for #259: a headless context is output-only.
 *
 * Built like doom_forcing_consumer.c, against the STAGED SDK through
 * `cc $(pkg-config --cflags --libs libgibson)` (see scripts/release/cleanroom.sh). It creates a
 * headless context and calls gibson_poll_event on it. The release contract calls the headless C
 * ABI output-only, so the poll must report GIBSON_EVENT_NONE with GIBSON_OK, promptly, without
 * opening or reading the process terminal. Before the fix the call read the real terminal: with
 * no terminal it returned GIBSON_ERR_IO, and on a terminal it consumed the user's keystrokes.
 *
 * Default mode (cleanroom, CI, no terminal): one headless poll with a 300 ms timeout must return
 * GIBSON_OK + GIBSON_EVENT_NONE in under 250 ms.
 *
 * `--pty` mode (run inside a pseudo-terminal by a harness that types "x\r" after READY): the
 * headless context polls for 1.5 s and must see nothing; then a real context created with
 * gibson_create_context must receive that same 'x', which shows the headless polls neither read
 * nor consumed it.
 *
 * Success prints `headless_input_consumer: OK` and exits 0; any violation exits nonzero with a
 * diagnostic on stderr.
 */
#define _POSIX_C_SOURCE 200809L
#include <gibson.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

static double now_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (double)t.tv_sec * 1e3 + (double)t.tv_nsec / 1e6;
}

static int fail(const char *m) {
    fprintf(stderr, "headless_input_consumer: FAIL: %s\n", m);
    return 1;
}

int main(int argc, char **argv) {
    int pty_mode = argc > 1 && strcmp(argv[1], "--pty") == 0;
    gibson_context_t *ctx = NULL;
    if (gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 20, 4, &ctx) != GIBSON_OK)
        return fail("create headless context");
    if (pty_mode) {
        printf("READY\n");
        fflush(stdout);
    }
    double until = now_ms() + (pty_mode ? 1500.0 : 0.0);
    do {
        gibson_event_t ev;
        memset(&ev, 0, sizeof ev);
        ev.event_type = -1;
        double t0 = now_ms();
        gibson_status_t st = gibson_poll_event(ctx, 300, &ev);
        double took = now_ms() - t0;
        if (st != GIBSON_OK) {
            fprintf(stderr, "status %d\n", (int)st);
            return fail("gibson_poll_event on a headless handle did not return GIBSON_OK");
        }
        if (ev.event_type != GIBSON_EVENT_NONE) {
            fprintf(stderr, "event_type %d key_code %u\n", (int)ev.event_type, ev.key_code);
            return fail("a headless handle reported an input event");
        }
        if (took >= 250.0) {
            fprintf(stderr, "took %.1f ms\n", took);
            return fail("a headless poll waited instead of returning at once");
        }
    } while (now_ms() < until);
    gibson_destroy_context(ctx);

    if (pty_mode) {
        gibson_context_t *real = NULL;
        if (gibson_create_context(GIBSON_MODE_INLINE, &real) != GIBSON_OK)
            return fail("create a terminal context");
        unsigned got = 0;
        double deadline = now_ms() + 5000.0;
        while (now_ms() < deadline && got == 0) {
            gibson_event_t ev;
            memset(&ev, 0, sizeof ev);
            if (gibson_poll_event(real, 100, &ev) == GIBSON_OK && ev.event_type == GIBSON_EVENT_KEY)
                got = ev.key_code;
        }
        gibson_destroy_context(real);
        if (got != 'x') {
            fprintf(stderr, "first key %u\n", got);
            return fail("the terminal context did not receive the key typed during the headless polls");
        }
    }
    printf("headless_input_consumer: OK\n");
    return 0;
}
