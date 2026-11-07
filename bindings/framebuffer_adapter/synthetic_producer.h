/* SPDX-License-Identifier: MIT OR Apache-2.0
 *
 * A rights-safe procedural producer for fb_adapter: a steerable tunnel with star dust and a ship
 * marker, drawn into a 320x200 palette-indexed framebuffer (the shape of a classic software
 * renderer). All art is generated here. The simulation is deterministic: its state depends only
 * on the seed, the number of tics and the key events delivered at each tic -- never on wall time,
 * presentation or the terminal size -- and synth_hash() is a 64-bit FNV-1a over that state.
 */
#ifndef SYNTHETIC_PRODUCER_H
#define SYNTHETIC_PRODUCER_H

#include "fb_adapter.h"

#include <stdint.h>

#define SYNTH_W 320
#define SYNTH_H 200
#define SYNTH_STARS 48
#define SYNTH_LOG 256

typedef struct synth_state {
    uint64_t tic;
    int32_t px, py, vx, vy; /* ship position / velocity, 1/256 pixel */
    uint32_t phase, spin;   /* tunnel depth / rotation */
    uint64_t rng;           /* xorshift64 */
    uint16_t pulse;         /* tics left of a pulse ring */
    uint8_t held;           /* bit 0 left, 1 right, 2 up, 3 down */
    uint16_t star_ang[SYNTH_STARS];
    int32_t star_z[SYNTH_STARS];
} synth_state;

typedef struct synth_key_log {
    uint64_t tic;
    uint32_t code;
    uint8_t pressed;
} synth_key_log;

typedef struct synth_producer {
    synth_state s;
    int ignore_input;                  /* negative control: keys are logged but change nothing */
    uint8_t pixels[SYNTH_H][SYNTH_W];  /* the framebuffer (palette indices) */
    uint8_t palette[256 * 3];
    uint8_t *tab_depth, *tab_angle, *tab_shade; /* (2W) x (2H) lookup tables */
    int16_t sin_tab[256];              /* sin(2 pi i / 256) * 1024 */
    synth_key_log log[SYNTH_LOG];
    unsigned nlog;
} synth_producer;

/* Allocates tables and seeds the state. 0 on success. */
int synth_init(synth_producer *p, uint64_t seed);
void synth_free(synth_producer *p);
/* Producer key codes are fb_adapter codes after the default map: arrows / WASD steer, space pulses. */
void synth_key(synth_producer *p, uint32_t code, int pressed);
void synth_tic(synth_producer *p);
/* Draws the current state into p->pixels. Reads the state, never writes it. */
void synth_render(synth_producer *p);
uint64_t synth_hash(const synth_producer *p);
/* key / tic / draw (render + fb_adapter_submit_indexed) for fb_adapter_run. */
fb_producer synth_producer_iface(synth_producer *p);

#endif /* SYNTHETIC_PRODUCER_H */
