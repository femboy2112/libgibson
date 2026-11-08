/* SPDX-License-Identifier: MIT OR Apache-2.0
 *
 * The synthetic producer: see synthetic_producer.h. Original procedural art; no game data.
 */
#include "synthetic_producer.h"

#include <math.h>
#include <stdlib.h>
#include <string.h>

#define TW (2 * SYNTH_W)
#define TH (2 * SYNTH_H)
#define FIX 256
#define PAL_STAR 240
#define PAL_SHIP 248
#define PAL_COCKPIT 249
#define PAL_FLAME_A 250
#define PAL_FLAME_B 251
#define PAL_PULSE 252

static uint64_t xorshift(uint64_t *x) {
    uint64_t v = *x;
    v ^= v << 13;
    v ^= v >> 7;
    v ^= v << 17;
    *x = v;
    return v;
}

static void set_pal(uint8_t *pal, int i, int r, int g, int b) {
    pal[3 * i] = (uint8_t)r;
    pal[3 * i + 1] = (uint8_t)g;
    pal[3 * i + 2] = (uint8_t)b;
}

static int lerp(int a, int b, int t, int n) { return a + (b - a) * t / n; }

static void build_palette(uint8_t *pal) {
    for (int sh = 0; sh < 15; sh++) {
        const int bright = 36 + sh * 219 / 14; /* far (centre) dark, near (edge) bright */
        for (int t = 0; t < 16; t++) {
            int r, g, b;
            if (t < 8) { /* violet -> magenta */
                r = lerp(70, 235, t, 7);
                g = lerp(24, 70, t, 7);
                b = lerp(130, 175, t, 7);
            } else {     /* deep blue -> amber */
                r = lerp(24, 255, t - 8, 7);
                g = lerp(48, 186, t - 8, 7);
                b = lerp(120, 72, t - 8, 7);
            }
            set_pal(pal, sh * 16 + t, r * bright / 255, g * bright / 255, b * bright / 255);
        }
    }
    set_pal(pal, PAL_STAR + 0, 110, 110, 150);
    set_pal(pal, PAL_STAR + 1, 160, 165, 210);
    set_pal(pal, PAL_STAR + 2, 215, 220, 255);
    set_pal(pal, PAL_STAR + 3, 255, 255, 255);
    for (int i = 244; i < 248; i++) set_pal(pal, i, 60 + (i - 244) * 30, 60 + (i - 244) * 30, 70 + (i - 244) * 30);
    set_pal(pal, PAL_SHIP, 40, 230, 255);
    set_pal(pal, PAL_COCKPIT, 255, 255, 255);
    set_pal(pal, PAL_FLAME_A, 255, 140, 30);
    set_pal(pal, PAL_FLAME_B, 255, 230, 90);
    set_pal(pal, PAL_PULSE, 255, 120, 255);
    for (int i = 253; i < 256; i++) set_pal(pal, i, 255, 255, 255);
}

static void respawn_star(synth_state *s, int i) {
    uint64_t r = xorshift(&s->rng);
    s->star_ang[i] = (uint16_t)(r & 0xFFu);
    s->star_z[i] = 500 + (int32_t)((r >> 8) % 500u);
}

int synth_init(synth_producer *p, uint64_t seed) {
    memset(&p->s, 0, sizeof p->s);
    p->ignore_input = 0;
    p->nlog = 0;
    p->tab_depth = malloc((size_t)TW * TH);
    p->tab_angle = malloc((size_t)TW * TH);
    p->tab_shade = malloc((size_t)TW * TH);
    if (!p->tab_depth || !p->tab_angle || !p->tab_shade) {
        synth_free(p);
        return -1;
    }
    const double pi = 3.14159265358979323846;
    for (int ty = 0; ty < TH; ty++)
        for (int tx = 0; tx < TW; tx++) {
            const double dx = tx - TW / 2 + 0.5;
            const double dy = (ty - TH / 2 + 0.5) * 1.2; /* 320x200 shown at 4:3: pixels are 1.2x tall */
            const double dist = sqrt(dx * dx + dy * dy);
            const size_t k = (size_t)ty * TW + tx;
            p->tab_depth[k] = (uint8_t)((int)(6144.0 / dist) & 255);
            p->tab_angle[k] = (uint8_t)((int)((atan2(dy, dx) + pi) * 256.0 / (2 * pi)) & 255);
            const int sh = (int)(dist / 16.0);
            p->tab_shade[k] = (uint8_t)(sh > 14 ? 14 : sh);
        }
    for (int i = 0; i < 256; i++) p->sin_tab[i] = (int16_t)lround(sin(2 * pi * i / 256.0) * 1024.0);
    build_palette(p->palette);
    p->s.rng = seed ? seed : 0x9E3779B97F4A7C15ull;
    p->s.px = (SYNTH_W / 2) * FIX;
    p->s.py = (SYNTH_H * 2 / 3) * FIX;
    for (int i = 0; i < SYNTH_STARS; i++) {
        respawn_star(&p->s, i);
        p->s.star_z[i] = 40 + (int32_t)(xorshift(&p->s.rng) % 900u);
    }
    return 0;
}

void synth_free(synth_producer *p) {
    free(p->tab_depth);
    free(p->tab_angle);
    free(p->tab_shade);
    p->tab_depth = p->tab_angle = p->tab_shade = NULL;
}

static int dir_bit(uint32_t code) {
    switch (code) {
    case FB_KEY_LEFT: case 'a': return 1;
    case FB_KEY_RIGHT: case 'd': return 2;
    case FB_KEY_UP: case 'w': return 4;
    case FB_KEY_DOWN: case 's': return 8;
    default: return 0;
    }
}

void synth_key(synth_producer *p, uint32_t code, int pressed) {
    if (p->nlog < SYNTH_LOG) {
        p->log[p->nlog].tic = p->s.tic;
        p->log[p->nlog].code = code;
        p->log[p->nlog].pressed = (uint8_t)(pressed != 0);
        p->nlog++;
    }
    if (p->ignore_input) return;
    const int bit = dir_bit(code);
    if (bit) {
        if (pressed) p->s.held |= (uint8_t)bit;
        else p->s.held &= (uint8_t)~bit;
    } else if (code == ' ' && pressed) {
        p->s.pulse = 24;
    }
}

static int32_t clamp32(int32_t v, int32_t lo, int32_t hi) { return v < lo ? lo : v > hi ? hi : v; }

void synth_tic(synth_producer *p) {
    synth_state *s = &p->s;
    const int ax = ((s->held >> 1) & 1) - (s->held & 1);
    const int ay = ((s->held >> 3) & 1) - ((s->held >> 2) & 1);
    s->vx = clamp32((s->vx + ax * 40) * 7 / 8, -4 * FIX, 4 * FIX);
    s->vy = clamp32((s->vy + ay * 40) * 7 / 8, -4 * FIX, 4 * FIX);
    s->px = clamp32(s->px + s->vx, 24 * FIX, (SYNTH_W - 24) * FIX);
    s->py = clamp32(s->py + s->vy, 16 * FIX, (SYNTH_H - 16) * FIX);
    s->phase += 3u + (s->pulse ? 3u : 0u);
    s->spin += (uint32_t)(1 + s->vx / FIX);
    for (int i = 0; i < SYNTH_STARS; i++) {
        s->star_z[i] -= 7 + (s->pulse ? 5 : 0);
        if (s->star_z[i] < 24) respawn_star(s, i);
    }
    if (s->pulse) s->pulse--;
    s->tic++;
}

static void put(synth_producer *p, int x, int y, uint8_t c) {
    if (x >= 0 && x < SYNTH_W && y >= 0 && y < SYNTH_H) p->pixels[y][x] = c;
}

/* A filled n x n block: features must stay visible after a ~8x nearest-neighbour downsample
   (320x200 into a 40x15 terminal). */
static void block(synth_producer *p, int x, int y, int n, uint8_t c) {
    for (int dy = 0; dy < n; dy++)
        for (int dx = 0; dx < n; dx++) put(p, x - n / 2 + dx, y - n / 2 + dy, c);
}

void synth_render(synth_producer *p) {
    const synth_state *s = &p->s;
    const int sx = s->px / FIX, sy = s->py / FIX;
    /* The ship banks the view: the window into the 2x lookup tables follows it at half rate. */
    const int wx = SYNTH_W / 2 + (sx - SYNTH_W / 2) / 2;
    const int wy = SYNTH_H / 2 + (sy - SYNTH_H / 2) / 2;
    const unsigned u0 = s->phase, v0 = s->spin;
    for (int y = 0; y < SYNTH_H; y++) {
        const size_t row = (size_t)(y + wy) * TW + (size_t)wx;
        for (int x = 0; x < SYNTH_W; x++) {
            const size_t k = row + (size_t)x;
            const unsigned u = (unsigned)p->tab_depth[k] + u0;
            const unsigned v = (unsigned)p->tab_angle[k] + v0;
            const unsigned tone = ((((u >> 4) ^ (v >> 5)) & 1u) ? 8u : 0u) + ((u >> 1) & 7u);
            p->pixels[y][x] = (uint8_t)(p->tab_shade[k] * 16u + tone);
        }
    }
    /* star dust streaming out of the tunnel centre */
    const int cx = TW / 2 - wx, cy = TH / 2 - wy;
    for (int i = 0; i < SYNTH_STARS; i++) {
        const int z = s->star_z[i];
        if (z > 760) continue; /* too far: would only clump at the centre */
        const int r = 9000 / (z > 1 ? z : 1);
        const int a = s->star_ang[i];
        const int x = cx + p->sin_tab[(a + 64) & 255] * r / 1024;
        const int y = cy + p->sin_tab[a] * r * 5 / 6 / 1024;
        int b = (1000 - z) / 250;
        b = b < 0 ? 0 : b > 3 ? 3 : b;
        block(p, x, y, 2 + b + (b == 3 ? 2 : 0), (uint8_t)(PAL_STAR + b));
    }
    /* pulse ring, 8 pixels thick */
    if (s->pulse) {
        const int rad = (24 - s->pulse) * 7 + 10;
        for (int i = 0; i < 256; i += 2)
            block(p, sx + p->sin_tab[(i + 64) & 255] * rad / 1024, sy + p->sin_tab[i] * rad * 5 / 6 / 1024, 8, PAL_PULSE);
    }
    /* the ship: a 41 x 21 chevron with a notch, a cockpit and a flickering engine */
    for (int dy = -12; dy <= 8; dy++)
        for (int dx = -20; dx <= 20; dx++) {
            const int half = (dy + 12) * 20 / 20; /* body widens 1 px per row */
            const int notch = dy > 0 ? dy * 8 / 8 : -1;
            if ((dx < 0 ? -dx : dx) <= half && !((dx < 0 ? -dx : dx) <= notch)) put(p, sx + dx, sy + dy, PAL_SHIP);
        }
    for (int dy = -5; dy <= 1; dy++)
        for (int dx = -3; dx <= 3; dx++)
            if (dx * dx + (dy + 2) * (dy + 2) <= 10) put(p, sx + dx, sy + dy, PAL_COCKPIT);
    const uint8_t flame = (s->tic & 1) ? PAL_FLAME_A : PAL_FLAME_B;
    const int flen = 6 + (int)(s->tic % 3) * 3;
    for (int dy = 2; dy < 2 + flen; dy++)
        for (int dx = -3; dx <= 3; dx++) put(p, sx + dx, sy + dy, (dy & 2) ? flame : PAL_FLAME_A);
}

/* ---- FNV-1a 64 over the state, field by field, little-endian ---- */
static void fnv(uint64_t *h, uint64_t v, int bytes) {
    for (int i = 0; i < bytes; i++) {
        *h ^= (uint8_t)(v >> (8 * i));
        *h *= 1099511628211ull;
    }
}

uint64_t synth_hash(const synth_producer *p) {
    const synth_state *s = &p->s;
    uint64_t h = 14695981039346656037ull;
    fnv(&h, s->tic, 8);
    fnv(&h, (uint32_t)s->px, 4);
    fnv(&h, (uint32_t)s->py, 4);
    fnv(&h, (uint32_t)s->vx, 4);
    fnv(&h, (uint32_t)s->vy, 4);
    fnv(&h, s->phase, 4);
    fnv(&h, s->spin, 4);
    fnv(&h, s->rng, 8);
    fnv(&h, s->pulse, 2);
    fnv(&h, s->held, 1);
    for (int i = 0; i < SYNTH_STARS; i++) {
        fnv(&h, s->star_ang[i], 2);
        fnv(&h, (uint32_t)s->star_z[i], 4);
    }
    return h;
}

/* ---- fb_producer glue ---- */
static void iface_key(void *u, uint32_t code, int pressed) { synth_key(u, code, pressed); }
static void iface_tic(void *u) { synth_tic(u); }
static fb_status iface_draw(void *u, fb_adapter *ad) {
    synth_producer *p = u;
    synth_render(p);
    return fb_adapter_submit_indexed(ad, &p->pixels[0][0], SYNTH_W, p->palette);
}

fb_producer synth_producer_iface(synth_producer *p) {
    fb_producer f;
    f.user = p;
    f.key = iface_key;
    f.tic = iface_tic;
    f.draw = iface_draw;
    return f;
}
