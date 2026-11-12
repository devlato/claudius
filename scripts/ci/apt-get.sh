#!/bin/sh
# apt-get on GitHub's Ubuntu runners, which fetch Ubuntu's packages from Azure's mirror of it. That mirror has stopped
# resolving, or answering, for hours at a time, and apt then waited out the job. So here every fetch gives up after
# 20 s and is tried twice more (in the job's later steps too), apt fetches from those of Azure's mirror, Ubuntu's own
# archive and kernel.org's mirror whose names resolve, in that order, and an update or an install that fails is tried
# once more without Azure's. apt checks Ubuntu's signatures on the packages whichever mirror serves them.
#
#   scripts/ci/apt-get.sh update
#   scripts/ci/apt-get.sh install <package>...

set -eu

AZURE=http://azure.archive.ubuntu.com/ubuntu/
UBUNTU=http://archive.ubuntu.com/ubuntu/
KERNEL=http://mirrors.edge.kernel.org/ubuntu/

# apt fetches from those of the mirrors given whose names resolve, the first first
mirrors() {
  list=$(mktemp)
  n=0
  for m in "$@"; do
    host=${m#http://}
    host=${host%%/*}
    if getent hosts "$host" > /dev/null; then
      n=$((n + 1))
      printf '%s\tpriority:%s\n' "$m" "$n" >> "$list"
    fi
  done
  if [ "$n" -eq 0 ]; then
    echo "::error title=Ubuntu packages::none of Ubuntu's mirrors resolves"
    exit 1
  fi
  sudo cp "$list" /etc/apt/apt-mirrors.txt
  rm -f "$list"
}

# without Azure's mirror, said once in the job's log
others() {
  echo "::warning title=Ubuntu packages::Azure's mirror of Ubuntu failed; fetching from Ubuntu's own archive or kernel.org's mirror"
  mirrors "$UBUNTU" "$KERNEL"
}

printf 'Acquire::http::Timeout "20";\nAcquire::https::Timeout "20";\nAcquire::Retries "2";\n' \
  | sudo tee /etc/apt/apt.conf.d/99claudius-ci > /dev/null

# An update that could not fetch Ubuntu's lists fails, which apt does not do by itself; one of the runner's other
# sources (Microsoft's, Google's) failing is only warned of, as apt does.
update() {
  status=0
  said=$(sudo apt-get update -qq 2>&1) || status=$?
  printf '%s\n' "$said"
  if printf '%s\n' "$said" | grep -Eq "Failed to fetch (mirror\+file:/etc/apt/apt-mirrors\.txt|$AZURE|$UBUNTU|$KERNEL)"; then
    status=1
  fi
  return "$status"
}

install() {
  sudo apt-get install -y -qq --no-install-recommends "$@"
}

case "${1:-}" in
  update)
    mirrors "$AZURE" "$UBUNTU" "$KERNEL"
    update || { others && update; }
    ;;
  install)
    shift
    install "$@" || { others && update && install "$@"; }
    ;;
  *)
    echo "usage: scripts/ci/apt-get.sh update | install <package>..." >&2
    exit 2
    ;;
esac
