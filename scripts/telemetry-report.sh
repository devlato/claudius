#!/bin/sh
# Sums claudius' usage statistics (docs/src/guide/privacy.md) from Workers Analytics Engine:
#   CLOUDFLARE_ACCOUNT_ID=... CLOUDFLARE_API_TOKEN=... sh scripts/telemetry-report.sh [days]
# The token needs Account Analytics read access. `days` defaults to 30.
set -eu

: "${CLOUDFLARE_ACCOUNT_ID:?set CLOUDFLARE_ACCOUNT_ID}"
: "${CLOUDFLARE_API_TOKEN:?set CLOUDFLARE_API_TOKEN, a token that can read Account Analytics}"
days=${1:-30}
case $days in
    '' | *[!0-9]*) echo "days has to be a number" >&2 && exit 2 ;;
esac
since="timestamp > NOW() - INTERVAL '$days' DAY"

query() {
    curl -fsS "https://api.cloudflare.com/client/v4/accounts/$CLOUDFLARE_ACCOUNT_ID/analytics_engine/sql" \
        -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" \
        --data-binary "$1 FORMAT TabSeparated"
}

echo "== started: command, os, arch, events (last $days days)"
query "SELECT blob2, blob4, blob5, SUM(_sample_interval) AS n FROM claudius_events WHERE blob1 = 'started' AND $since GROUP BY blob2, blob4, blob5 ORDER BY n DESC"
echo
echo "== started: version, events"
query "SELECT blob3, SUM(_sample_interval) AS n FROM claudius_events WHERE blob1 = 'started' AND $since GROUP BY blob3 ORDER BY n DESC"
echo
echo "== setup: outcome, os, events"
query "SELECT blob6, blob4, SUM(_sample_interval) AS n FROM claudius_events WHERE blob1 = 'setup' AND $since GROUP BY blob6, blob4 ORDER BY n DESC"
echo
echo "== panic: where, version, events"
query "SELECT blob6, blob3, SUM(_sample_interval) AS n FROM claudius_events WHERE blob1 = 'panic' AND $since GROUP BY blob6, blob3 ORDER BY n DESC"
