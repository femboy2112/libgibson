/* Issue #274: gibson_poll_event_kind / gibson_push_event_kind through the C ABI.
 *
 * GIBSON_KEY_* occupy 0x1001-0x100D, which are also Unicode scalar values (Myanmar letters).
 * For every value in that range this pushes it once as a typed character and once as a special
 * key, and requires the kind-aware poll to return each with its own kind. It also checks the
 * legacy functions are unchanged: gibson_push_event reads the range as special keys, and
 * gibson_poll_event reports a typed letter by its code alone. Prints "key_kind: OK" or exits 1.
 */
#include "gibson.h"
#include <stdio.h>
#include <string.h>

static int fail(const char *what, unsigned code) {
    fprintf(stderr, "key_kind: FAIL %s (code 0x%x)\n", what, code);
    return 1;
}

static gibson_event_t key(uint32_t code) {
    gibson_event_t ev;
    memset(&ev, 0, sizeof ev);
    ev.event_type = GIBSON_EVENT_KEY;
    ev.key_code = code;
    return ev;
}

int main(void) {
    gibson_context_t *ctx = NULL;
    if (gibson_create_headless_context(GIBSON_MODE_FULLSCREEN, 40, 10, &ctx) != GIBSON_OK)
        return fail("create headless context", 0);
    int rc = 0;
    for (uint32_t cp = 0x1001; cp <= 0x100D && rc == 0; cp++) {
        gibson_event_t ev = key(cp), out;
        uint32_t kind = 0xFFFFFFFFu;
        if (gibson_push_event_kind(ctx, &ev, GIBSON_KEY_KIND_CHAR) != GIBSON_OK) rc = fail("push CHAR", cp);
        else if (gibson_push_event_kind(ctx, &ev, GIBSON_KEY_KIND_SPECIAL) != GIBSON_OK) rc = fail("push SPECIAL", cp);
        else if (gibson_poll_event_kind(ctx, 0, &out, &kind) != GIBSON_OK
                 || out.event_type != GIBSON_EVENT_KEY || out.key_code != cp || kind != GIBSON_KEY_KIND_CHAR)
            rc = fail("typed letter must poll back CHAR", cp);
        else if (gibson_poll_event_kind(ctx, 0, &out, &kind) != GIBSON_OK
                 || out.event_type != GIBSON_EVENT_KEY || out.key_code != cp || kind != GIBSON_KEY_KIND_SPECIAL)
            rc = fail("special key must poll back SPECIAL", cp);
        /* Legacy push: the range is the special keys. */
        else if (gibson_push_event(ctx, &ev) != GIBSON_OK
                 || gibson_poll_event_kind(ctx, 0, &out, &kind) != GIBSON_OK || kind != GIBSON_KEY_KIND_SPECIAL)
            rc = fail("legacy push must stay SPECIAL", cp);
        /* Legacy poll: a typed letter is reported by its code alone (the ambiguity it keeps). */
        else if (gibson_push_event_kind(ctx, &ev, GIBSON_KEY_KIND_CHAR) != GIBSON_OK
                 || gibson_poll_event(ctx, 0, &out) != GIBSON_OK || out.key_code != cp)
            rc = fail("legacy poll must report the code unchanged", cp);
    }
    if (rc == 0) {
        gibson_event_t ev = key('a');
        if (gibson_push_event_kind(ctx, &ev, GIBSON_KEY_KIND_SPECIAL) != GIBSON_ERR_INVALID_PARAM)
            rc = fail("SPECIAL with a non-special code must be refused", 'a');
        else if (gibson_push_event_kind(ctx, &ev, 7) != GIBSON_ERR_INVALID_PARAM)
            rc = fail("an unknown kind must be refused", 'a');
    }
    gibson_destroy_context(ctx);
    if (rc == 0) printf("key_kind: OK (13 collision codes x 2 kinds, legacy push/poll unchanged)\n");
    return rc;
}
