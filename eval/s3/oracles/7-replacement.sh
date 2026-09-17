#!/usr/bin/env bash
# S3 task 7 (replacement) oracle: a command that never asks the time must not read the wall clock.
#
# Behavioural. A small shim interposes clock_gettime(CLOCK_REALTIME) and gettimeofday and logs every
# call (DYLD_INSERT_LIBRARIES on macOS, LD_PRELOAD on Linux). `ucal now` must register a read, or
# the shim is not injected and the result means nothing (exit 2). `ucal datum` and `ucal ladder`
# never ask the time, so any read there breaks the prerequisite (exit 1).
#
# usage: 7-replacement.sh <ucal-repo> [ucal-binary]
# exit: 0 intact, 1 violated, 2 harness error
set -u
repo=${1:?usage: 7-replacement.sh <ucal-repo> [ucal-binary]}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export HOME="$work/home"
mkdir -p "$HOME"

bin=${2:-}
if [ -z "$bin" ]; then
  target=${CARGO_TARGET_DIR:-$repo/target}
  (cd "$repo" && cargo build -q -p ucal) || { echo "oracle 7: build failed" >&2; exit 2; }
  bin="$target/debug/ucal"
fi
[ -x "$bin" ] || { echo "oracle 7: no binary at $bin" >&2; exit 2; }

cat > "$work/clockspy.c" <<'EOF'
#define _GNU_SOURCE
#include <time.h>
#include <sys/time.h>
#include <stdlib.h>
#include <fcntl.h>
#include <unistd.h>
static void note(void) {
  const char *p = getenv("CLOCKSPY_LOG");
  if (!p) return;
  int fd = open(p, O_WRONLY | O_CREAT | O_APPEND, 0644);
  if (fd >= 0) { (void)!write(fd, "realtime\n", 9); close(fd); }
}
#ifdef __APPLE__
int spy_clock_gettime(clockid_t id, struct timespec *ts) { if (id == CLOCK_REALTIME) note(); return clock_gettime(id, ts); }
int spy_gettimeofday(struct timeval *tv, void *tz) { note(); return gettimeofday(tv, tz); }
typedef struct { const void *repl; const void *orig; } interpose_t;
__attribute__((used)) static const interpose_t interposers[] __attribute__((section("__DATA,__interpose"))) = {
  { (const void *)spy_clock_gettime, (const void *)clock_gettime },
  { (const void *)spy_gettimeofday, (const void *)gettimeofday },
};
#else
#include <dlfcn.h>
int clock_gettime(clockid_t id, struct timespec *ts) {
  static int (*real)(clockid_t, struct timespec *);
  if (!real) real = dlsym(RTLD_NEXT, "clock_gettime");
  if (id == CLOCK_REALTIME) note();
  return real(id, ts);
}
int gettimeofday(struct timeval *tv, void *tz) {
  static int (*real)(struct timeval *, void *);
  if (!real) real = dlsym(RTLD_NEXT, "gettimeofday");
  note();
  return real(tv, tz);
}
#endif
EOF
if [ "$(uname)" = Darwin ]; then
  cc -dynamiclib -o "$work/clockspy.so" "$work/clockspy.c" || exit 2
  inject() { DYLD_INSERT_LIBRARIES="$work/clockspy.so" "$@"; }
else
  cc -shared -fPIC -o "$work/clockspy.so" "$work/clockspy.c" -ldl || exit 2
  inject() { LD_PRELOAD="$work/clockspy.so" "$@"; }
fi

reads() {
  rm -f "$work/log"
  CLOCKSPY_LOG="$work/log" inject "$bin" "$@" >/dev/null 2>&1
  if [ -f "$work/log" ]; then wc -l < "$work/log" | tr -d ' '; else echo 0; fi
}

if [ "$(reads now)" -lt 1 ]; then
  echo "oracle 7: \`ucal now\` registered no clock read; the shim is not injected" >&2
  exit 2
fi
status=0
for cmd in datum ladder; do
  n=$(reads "$cmd")
  if [ "$n" -gt 0 ]; then
    echo "oracle 7: \`ucal $cmd\` read the wall clock $n time(s); it never asks the time"
    status=1
  fi
done
[ $status -eq 0 ] && echo "oracle 7: commands that never ask the time read no clock"
exit $status
