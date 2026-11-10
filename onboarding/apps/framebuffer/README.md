# Software framebuffer consumer

This small, headless C program submits a 4×4 ARGB32 framebuffer through the
repository's `fb_adapter`, then reads the rendered 4×2 cell grid through the
public C ABI. It checks every half-block glyph and its foreground/background RGB,
rejects a short source pitch, and checks that the adapter copied the caller's
pixels. It uses no terminal and does not test interactive input or performance.

Dependencies: a C11 compiler, standard C/POSIX and `libm`, a staged Linux
LibGibson development SDK (ABI v1, headers and shared library), and the matching
`fb_adapter.c` / `fb_adapter.h` sources. The adapter is a repository prototype;
it is **not** bundled in the native SDK. Its sources are MIT OR Apache-2.0.

Copy this consumer directory and the two adapter files outside the checkout,
then use the staged SDK. For example, from the repository root:

```sh
export LAB="$(mktemp -d)"
cp -R onboarding/apps/framebuffer "$LAB/framebuffer"
mkdir "$LAB/adapter"
cp bindings/framebuffer_adapter/fb_adapter.c bindings/framebuffer_adapter/fb_adapter.h "$LAB/adapter/"
export GIBSON_FB_ADAPTER="$LAB/adapter"
export GIBSON_SDK=/path/to/staged/sdk
"$LAB/framebuffer/smoke.sh"
```

The script builds in a separate temporary directory and uses only those staged
headers/library and copied adapter files. It runs the program once normally, then
checks that a deliberately wrong expected color fails at the expected cell. The
`--wrong-oracle` argument is a test control, not an alternate rendering mode.
The underlying adapter and its scaling, input and lifecycle contracts are
explained in `bindings/framebuffer_adapter/README.md` in the repository.
