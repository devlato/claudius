#!/bin/sh
# apt-get on GitHub's Ubuntu runners, which fetch Ubuntu's packages from Azure's mirror of it. That mirror has stopped
# resolving, or answering, for hours at a time, and apt then waited out the job; and it has slowed to a trickle for
# many minutes (19 November 2026: four small packages took six minutes, and valgrind was cut off at ten). So here every
# fetch gives up after 20 s without data and is tried twice more (in the job's later steps too); apt fetches from
# Azure's mirror, Ubuntu's own archive and kernel.org's mirror, of those whose names resolve, the fastest first, as
# each serves Ubuntu's release file; and an update or an install that fails is tried once more without Azure's. apt
# checks Ubuntu's signatures on the packages whichever mirror serves them. A job that may use a cache installs the
# packages that the last run on main for its runner image kept (`kept`), with no mirror at all.
#
#   scripts/ci/apt-get.sh update
#   scripts/ci/apt-get.sh install <package>...
#   scripts/ci/apt-get.sh kept <directory> <package>...

set -eu

AZURE=http://azure.archive.ubuntu.com/ubuntu/
UBUNTU=http://archive.ubuntu.com/ubuntu/
KERNEL=http://mirrors.edge.kernel.org/ubuntu/
# the lists were fetched in this job
FETCHED=/tmp/claudius-apt-lists

# apt fetches from those of the mirrors given whose names resolve, the fastest first, as each serves Ubuntu's release
# file within 5 s (one that does not comes last)
mirrors() {
  list=$(mktemp)
  speeds=$(mktemp)
  codename=$(sed -n 's/^VERSION_CODENAME=//p' /etc/os-release | tr -d '"')
  for m in "$@"; do
    host=${m#http://}
    host=${host%%/*}
    if getent hosts "$host" > /dev/null; then
      speed=$(curl -s -o /dev/null --max-time 5 -w '%{speed_download}' "${m}dists/$codename/InRelease" || true)
      printf '%s %s\n' "${speed:-0}" "$m" >> "$speeds"
    fi
  done
  if [ ! -s "$speeds" ]; then
    echo "::error title=Ubuntu packages::none of Ubuntu's mirrors resolves"
    exit 1
  fi
  n=0
  sort -rn -k1,1 "$speeds" | while read -r speed m; do
    n=$((n + 1))
    printf '%s\tpriority:%s\n' "$m" "$n" >> "$list"
    echo "$m: $(awk -v s="$speed" 'BEGIN { printf "%.0f", s / 1024 }') KB/s"
  done
  sudo cp "$list" /etc/apt/apt-mirrors.txt
  rm -f "$list" "$speeds"
}

# without Azure's mirror, said once in the job's log
others() {
  echo "::warning title=Ubuntu packages::Azure's mirror of Ubuntu failed; fetching from Ubuntu's own archive or kernel.org's mirror"
  mirrors "$UBUNTU" "$KERNEL"
}

# apt reads its settings in the order of their files' names, the runner's own `zz-retries` too: these go after it
printf 'Acquire::http::Timeout "20";\nAcquire::https::Timeout "20";\nAcquire::Retries "2";\n' \
  | sudo tee /etc/apt/apt.conf.d/zzz-claudius-ci > /dev/null

# An update that could not fetch Ubuntu's lists fails, which apt does not do by itself; one of the runner's other
# sources (Microsoft's, Google's) failing is only warned of, as apt does.
update() {
  status=0
  said=$(sudo apt-get update -qq 2>&1) || status=$?
  printf '%s\n' "$said"
  if printf '%s\n' "$said" | grep -Eq "Failed to fetch (mirror\+file:/etc/apt/apt-mirrors\.txt|$AZURE|$UBUNTU|$KERNEL)"; then
    status=1
  fi
  if [ "$status" -eq 0 ]; then
    touch "$FETCHED"
  fi
  return "$status"
}

# Ubuntu's lists, fetched from the fastest mirror, or without Azure's when that fails
refresh() {
  mirrors "$AZURE" "$UBUNTU" "$KERNEL"
  update || { others && update; }
}

install() {
  sudo apt-get install -y -qq --no-install-recommends "$@"
}

# what installing the packages would fetch, into the directory given first
fetch() {
  into=$1
  shift
  sudo apt-get install -y -qq --no-install-recommends --download-only -o Dir::Cache::archives="$into" "$@"
}

# The packages that the last run on main for this runner image kept in `dir` (actions/cache), installed from there
# with no mirror at all; or, without them, fetched into `dir` for main's run to keep, and installed. `.kept` says the
# folder is whole: what was fetched installed as it is (which is no package at all where the image has them all). A
# fetch that dpkg does not install is installed by apt instead and left unmarked, so that main does not keep a folder
# no later run could install.
kept() {
  dir=$1
  shift
  if [ -f "$dir/.kept" ]; then
    if ! ls "$dir"/*.deb > /dev/null 2>&1 || sudo dpkg -i "$dir"/*.deb; then
      echo "installed what the last run on main kept for this runner image: $*"
      return 0
    fi
    echo "::warning title=Ubuntu packages::the packages kept for this runner image did not install; fetching them"
  fi
  rm -rf "$dir"
  mkdir -p "$dir/partial"
  refresh
  fetch "$dir" "$@" || { others && update && fetch "$dir" "$@"; }
  sudo rm -rf "$dir/partial" "$dir/lock"
  sudo chown -R "$(id -u):$(id -g)" "$dir"
  if ! ls "$dir"/*.deb > /dev/null 2>&1; then
    touch "$dir/.kept"
  elif sudo dpkg -i "$dir"/*.deb; then
    touch "$dir/.kept"
  else
    install "$@"
  fi
}

case "${1:-}" in
  update)
    refresh
    ;;
  install)
    shift
    # the lists, when no update in this job fetched them (one whose other packages were kept)
    [ -f "$FETCHED" ] || refresh
    install "$@" || { others && update && install "$@"; }
    ;;
  kept)
    shift
    kept "$@"
    ;;
  *)
    echo "usage: scripts/ci/apt-get.sh update | install <package>... | kept <directory> <package>..." >&2
    exit 2
    ;;
esac
