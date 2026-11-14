#!/bin/sh
# Runs cargo xtask ci on a checkout of claudius the way GitHub's runners see a machine: no sample packs and no models,
# in a home of its own, because what is installed here changes what some tests do. It remembers the tree it passed
# on, so that a push of the same tree, which .claude/hooks/guard.sh holds until this has passed, goes out at once.
#
#   .claude/hooks/ci.sh [checkout]    the current one by default; the log goes to target/claude-ci.log in it

cd "${1:-.}" 2> /dev/null || {
    echo "ci.sh: cannot open ${1:-.}" >&2
    exit 1
}
top=$(git rev-parse --show-toplevel 2> /dev/null) || {
    echo "ci.sh: $(pwd) is not a checkout" >&2
    exit 1
}
cd "$top" || exit 1

if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
    echo "ci.sh: $top has uncommitted changes. Commit or stash them first, so the checks run on what gets pushed." >&2
    exit 1
fi
tree=$(git rev-parse 'HEAD^{tree}') || exit 1
passed=$(git rev-parse --path-format=absolute --git-path claude-ci-passed)
lock=$(git rev-parse --path-format=absolute --git-path claude-ci.lock)

# one run at a time in a checkout: a second one waits for the first, then finds its result
until mkdir "$lock" 2> /dev/null; do
    pid=$(cat "$lock/pid" 2> /dev/null)
    if [ -n "$pid" ] && ! kill -0 "$pid" 2> /dev/null; then
        rm -rf "$lock"
    else
        sleep 5
    fi
done
echo $$ > "$lock/pid"
trap 'rm -rf "$lock"' EXIT
trap 'exit 1' HUP INT TERM

if [ "$(cat "$passed" 2> /dev/null)" = "$tree" ]; then
    echo "cargo xtask ci has passed on this tree already"
    exit 0
fi

mkdir -p target
log=$top/target/claude-ci.log
home=${TMPDIR:-/tmp}/claudius-ci-home
cargo_home=${CARGO_HOME:-$HOME/.cargo}
rustup_home=${RUSTUP_HOME:-$HOME/.rustup}
mkdir -p "$home"
echo "running cargo xtask ci in $top; the log is $log"
if HOME=$home CARGO_HOME=$cargo_home RUSTUP_HOME=$rustup_home PATH=$cargo_home/bin:$PATH CLAUDIUS_PACKS=/nonexistent \
    cargo xtask ci > "$log" 2>&1; then
    printf '%s\n' "$tree" > "$passed"
    echo "cargo xtask ci passed"
else
    echo "cargo xtask ci failed. The end of $log:" >&2
    tail -n 60 "$log" >&2
    exit 1
fi
