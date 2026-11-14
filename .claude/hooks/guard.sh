#!/bin/sh
# Claude Code runs this before each command and edit it makes in claudius (.claude/settings.json), with the call as
# JSON on stdin, to hold it to the rules of AGENTS.md that a script can check. Exit 2 refuses the call and stderr
# tells Claude why; a call this script cannot judge goes through.
set -f

refuse() {
    printf '%s\n' "$@" >&2
    exit 2
}

command -v jq > /dev/null 2>&1 || exit 0
input=$(cat)

field() {
    printf '%s' "$input" | jq -r "$1 // empty" 2> /dev/null
}

cwd=$(field .cwd)
[ -n "$cwd" ] && [ -d "$cwd" ] || cwd=${CLAUDE_PROJECT_DIR:-$PWD}
project=${CLAUDE_PROJECT_DIR:-$cwd}
here=$(cd "$(dirname "$0")" && pwd)

# the git directory that every worktree of the repository holding $1 shares
common_dir() {
    git -C "$1" rev-parse --path-format=absolute --git-common-dir 2> /dev/null
}

# true when $1 is in claudius: this checkout or another worktree of it
ours() {
    theirs=$(common_dir "$1") && [ -n "$theirs" ] && [ "$theirs" = "$(common_dir "$project")" ]
}

# true while the checkout $1 is part way through a rebase, merge, cherry-pick or revert
midway() {
    for state in rebase-merge rebase-apply MERGE_HEAD CHERRY_PICK_HEAD REVERT_HEAD; do
        [ -e "$(git -C "$1" rev-parse --path-format=absolute --git-path "$state")" ] && return 0
    done
    return 1
}

types="feat, fix, docs, refactor, perf, test, build, ci or chore"

edit() {
    file=$(field '.tool_input.file_path // .tool_input.notebook_path')
    [ -n "$file" ] || exit 0
    case $file in
        /*) ;;
        *) file=$cwd/$file ;;
    esac
    dir=$(dirname "$file")
    while [ ! -d "$dir" ]; do dir=$(dirname "$dir"); done
    top=$(git -C "$dir" rev-parse --show-toplevel 2> /dev/null) || exit 0
    ours "$top" || exit 0
    git -C "$top" check-ignore -q -- "$file" && exit 0
    midway "$top" && exit 0
    branch=$(git -C "$top" symbolic-ref --quiet --short HEAD) ||
        refuse "AGENTS.md: work on a branch. $top is on a detached commit; make one first:" \
            "git -C $top switch -c <type>/<topic>, the type being $types."
    case $branch in
        feat/?* | fix/?* | docs/?* | refactor/?* | perf/?* | test/?* | build/?* | ci/?* | chore/?*) exit 0 ;;
        release/?* | hotfix/?*) exit 0 ;;
        main) refuse "AGENTS.md: never work on main. Make a branch first:" \
            "git -C $top switch -c <type>/<topic>, the type being $types." ;;
        *) refuse "AGENTS.md: a branch is named <type>/<topic>, and $branch is not. Rename it first:" \
            "git -C $top branch -m <type>/<topic>, the type being $types." ;;
    esac
}

# the folder of a quoted `cd "..." &&` that starts the command, which the words below have lost
leading_cd() {
    printf '%s\n' "$cmd" | sed -n \
        -e 's/^[[:space:]]*cd[[:space:]][[:space:]]*"\([^"]*\)".*/\1/p' \
        -e "s/^[[:space:]]*cd[[:space:]][[:space:]]*'\\([^']*\\)'.*/\\1/p" | head -n 1
}

# one simple command, as words; follows `cd`, refuses what AGENTS.md forbids, and marks a push for the checks
look() {
    while [ $# -gt 0 ]; do
        case $1 in
            *=* | env | command | exec | nohup | time) shift ;;
            *) break ;;
        esac
    done
    [ $# -gt 0 ] || return 0
    case $1 in
        cd)
            target=${2:-$HOME}
            [ "$target" = Q ] && target=$(leading_cd)
            case $target in
                '') ;;
                /*) at=$target ;;
                '~'*) at=$HOME${target#\~} ;;
                *) at=$at/$target ;;
            esac
            return 0
            ;;
        gh)
            if [ "${2:-}" = pr ] && [ "${3:-}" = create ]; then
                gate=1
                gate_dir=$at
            fi
            return 0
            ;;
        git) shift ;;
        *) return 0 ;;
    esac
    where=$at
    while [ $# -gt 0 ]; do
        case $1 in
            -C)
                case ${2:-} in
                    '' | Q) ;;
                    /*) where=$2 ;;
                    *) where=$where/$2 ;;
                esac
                shift
                [ $# -gt 0 ] && shift
                ;;
            -c)
                shift
                [ $# -gt 0 ] && shift
                ;;
            -*) shift ;;
            *) break ;;
        esac
    done
    [ $# -gt 0 ] || return 0
    sub=$1
    shift
    case $sub in
        commit)
            for word in "$@"; do
                case $word in
                    --) break ;;
                    --*) ;;
                    -*n*) refuse "AGENTS.md: no --no-verify, and git commit -n is --no-verify." \
                        "If a hook is wrong, say so in the pull request." ;;
                esac
            done
            ;;
        push)
            skip=
            for word in "$@"; do
                case $word in
                    --tags | refs/tags/v* | v[0-9]*)
                        refuse "AGENTS.md: v* tags change only through the release pull request that release-pr.yml opens."
                        ;;
                    -d | --delete | -n | --dry-run | :*) skip=1 ;;
                esac
            done
            if [ -z "$skip" ]; then
                gate=1
                gate_dir=$where
            fi
            ;;
    esac
}

run() {
    cmd=$(field .tool_input.command)
    [ -n "$cmd" ] || exit 0
    # a line ends a command; quoted text (messages, titles, bodies) becomes one placeholder word, so that its words
    # are not taken for options
    bare=$(printf '%s' "$cmd" | tr '\n' ';' | sed -e "s/'[^']*'/ Q /g" -e 's/"[^"]*"/ Q /g')
    case " $bare " in
        *" --no-verify"*) refuse "AGENTS.md: no --no-verify. If a hook is wrong, say so in the pull request." ;;
    esac
    at=$cwd
    gate=
    gate_dir=
    parts=$(printf '%s\n' "$bare" | awk '{ gsub(/&&|\|\||[;|&()]/, "\n"); print }')
    newline='
'
    old_ifs=$IFS
    IFS=$newline
    for part in $parts; do
        IFS=$old_ifs
        # shellcheck disable=SC2086 # split into words on purpose; globbing is off
        look $part
        IFS=$newline
    done
    IFS=$old_ifs
    [ -n "$gate" ] || exit 0
    top=$(git -C "$gate_dir" rev-parse --show-toplevel 2> /dev/null) || exit 0
    ours "$top" || exit 0
    report=$("$here/ci.sh" "$top" 2>&1) ||
        refuse "$report" "" "AGENTS.md: cargo xtask ci passes before every push. Fix what it found, commit, and push again."
    exit 0
}

case $(field .tool_name) in
    Bash) run ;;
    Edit | Write | MultiEdit | NotebookEdit) edit ;;
esac
exit 0
