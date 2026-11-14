# Decision 0004: release names

| | |
|---|---|
| Status | **Decided 2026-11-13.** The owner chose the scheme. The release coordinator settled the details under the owner's delegation. |
| Contract | [Release Contract §1](../RELEASE_CONTRACT.md#1-versioning-model); [`RELEASING.md`](../RELEASING.md) §6 |
| First use | 0.7.1: "LibGibson v0.7.1 — clear SIGBUS 1", [released](https://github.com/femboy2112/libgibson/releases/tag/v0.7.1) 2026-11-13 (tag → `835796e`) |
| Recorded | 2026-11-13 |

## The scheme

Every release `MAJOR.MINOR.PATCH` also gets a name of the form **`<utility> <signal> <patch>`**.

- **Major → a common Linux shell utility.** 0 is `clear`. 1 is `cron`, which is reserved and
  takes effect only with the owner's 1.0 go. Later majors get a utility when they are decided.
- **Minor → the Linux signal with that number**, from the table below.
- **Patch → the patch number, always written.** "clear SIGBUS 0", not "clear SIGBUS".

Example: **0.7.11 = "clear SIGBUS 11"**.

## Where the name appears

- In the **GitHub release title**: `LibGibson v0.7.1 — clear SIGBUS 1`.
- In the **annotated tag's message**, on its first line.

The **tag ref itself stays plain SemVer** (`v0.7.1`, `bindings/go/v0.7.1`). The name is never
part of a tag name, a version string, `Cargo.toml`, `pyproject.toml` or a package filename.
Under SemVer, `v0.7.1-clear` would parse as a *pre-release* of 0.7.1. Nothing in the build reads
the name.

Pre-releases (`-alpha.N`, `-rc.N`) carry no name. The name belongs to the release itself.

## Signal table

The numbers are the x86-64 Linux kernel's, as `kill -l` prints them there. Numbering differs on
other architectures. For example, SIGBUS is 10 on MIPS and SPARC. The x86-64 table is the
reference regardless.

| Minor | Signal | Minor | Signal | Minor | Signal | Minor | Signal |
|---|---|---|---|---|---|---|---|
| 0 | SIG0 | 8 | SIGFPE | 16 | SIGSTKFLT | 24 | SIGXCPU |
| 1 | SIGHUP | 9 | SIGKILL | 17 | SIGCHLD | 25 | SIGXFSZ |
| 2 | SIGINT | 10 | SIGUSR1 | 18 | SIGCONT | 26 | SIGVTALRM |
| 3 | SIGQUIT | 11 | SIGSEGV | 19 | SIGSTOP | 27 | SIGPROF |
| 4 | SIGILL | 12 | SIGUSR2 | 20 | SIGTSTP | 28 | SIGWINCH |
| 5 | SIGTRAP | 13 | SIGPIPE | 21 | SIGTTIN | 29 | SIGIO |
| 6 | SIGABRT | 14 | SIGALRM | 22 | SIGTTOU | 30 | SIGPWR |
| 7 | SIGBUS | 15 | SIGTERM | 23 | SIGURG | 31 | SIGSYS |

- **Minor 0 is `SIG0`**, the null signal. `kill -0` sends no signal and asks only whether the
  process exists, which suits an `x.0` line.
- **Minor 32 and up is `SIGRTMIN+<minor − 32>`**, taking the kernel's SIGRTMIN as 32. For
  example, 0.33 is `SIGRTMIN+1`. glibc reserves 32 and 33, so `kill -l` shows `RTMIN` at 34.
  This scheme counts from the kernel's 32 regardless.
- The table above was checked against `kill -l 1` … `kill -l 31` on x86-64 Linux.

## Release names

The names for v0.1.0 through v0.7.0 were assigned after the fact and live only in this table. From v0.7.1 on, each release carries its name in its title and tag message.
**Published releases and tags are not edited**: v0.7.0 keeps the title it was published with.

| Release | Name |
|---|---|
| v0.1.0 | clear SIGHUP 0 |
| v0.1.1 | clear SIGHUP 1 |
| v0.2.0 | clear SIGINT 0 |
| v0.2.1 | clear SIGINT 1 |
| v0.2.2 | clear SIGINT 2 |
| v0.2.3 | clear SIGINT 3 |
| v0.2.4 | clear SIGINT 4 |
| v0.2.5 | clear SIGINT 5 |
| v0.3.0 | clear SIGQUIT 0 |
| v0.3.1 | clear SIGQUIT 1 |
| v0.4.0 | clear SIGILL 0 |
| v0.5.0 | clear SIGTRAP 0 |
| v0.6.0 | clear SIGABRT 0 |
| v0.7.0 | clear SIGBUS 0 |
| v0.7.1 | clear SIGBUS 1 (first named release) |

The pre-releases `v0.7.0-alpha.1` through `v0.7.0-alpha.6` have no name.

## Releases from now on

From 0.7.1 on, the release procedure adds two steps:
1. Look up the name in the table above.
2. Use it in the release title and on the first line of the tag message:
   `LibGibson v<version> — <utility> <signal> <patch>`.

Add each named release to the table in the post-release docs PR.
