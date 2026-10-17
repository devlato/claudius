#!/bin/sh
# Configures the GitHub repository once it exists: merging, branch protection, security features, the topics and
# the homepage. Every call sets a value rather than adding one, so running it again is safe. It needs `gh` logged in
# as an admin of the repository, and main must already have a commit (branch protection applies to a branch).
#
#   scripts/github-setup.sh
#   REVIEWS=1 scripts/github-setup.sh       # once there is a second maintainer to approve pull requests
#
# Environment:
#   REPO            owner/name (default: devlato/claudius)
#   REVIEWS         approving reviews a pull request needs (default: 0, for a single maintainer; a pull request is
#                   still required, and nobody can approve their own)
#   ENFORCE_ADMINS  whether the rules bind admins too (default: true, so that an admin's token, or an agent using
#                   it, cannot push to main either)
#   HOMEPAGE        the website, set as the repository's homepage when given
#   DESCRIPTION     the repository's description, set when given (otherwise the one on GitHub stays)

set -eu

REPO=${REPO:-devlato/claudius}
REVIEWS=${REVIEWS:-0}
ENFORCE_ADMINS=${ENFORCE_ADMINS:-true}
HOMEPAGE=${HOMEPAGE:-}
DESCRIPTION=${DESCRIPTION:-}
TOPICS="music music-production beatmaker audio rust egui wasm mcp strudel rhai"

step() {
    printf '\n== %s\n' "$*"
}

gh auth status >/dev/null 2>&1 || {
    echo "log in first: gh auth login" >&2
    exit 1
}

step "merging: squash only, the PR title as the commit title, branches deleted after merging, auto-merge"
set --
[ -n "$DESCRIPTION" ] && set -- "$@" -f description="$DESCRIPTION"
[ -n "$HOMEPAGE" ] && set -- "$@" -f homepage="$HOMEPAGE"
gh api --method PATCH "repos/$REPO" --silent "$@" \
    -F has_wiki=false \
    -F allow_squash_merge=true \
    -F allow_merge_commit=false \
    -F allow_rebase_merge=false \
    -f squash_merge_commit_title=PR_TITLE \
    -f squash_merge_commit_message=PR_BODY \
    -F delete_branch_on_merge=true \
    -F allow_auto_merge=true \
    -F allow_update_branch=true

step "topics: $TOPICS"
set --
for topic in $TOPICS; do
    set -- "$@" -f "names[]=$topic"
done
gh api --method PUT "repos/$REPO/topics" --silent "$@"

step "actions: a read-only GITHUB_TOKEN by default; workflows may open pull requests (release-pr.yml does)"
gh api --method PUT "repos/$REPO/actions/permissions/workflow" --silent \
    -f default_workflow_permissions=read \
    -F can_approve_pull_request_reviews=true

step "branch protection on main: \"ci ok\" passes, $REVIEWS approving review(s), linear history, no force pushes"
gh api --method PUT "repos/$REPO/branches/main/protection" --silent --input - <<EOF
{
  "required_status_checks": { "strict": false, "checks": [{ "context": "ci ok" }] },
  "enforce_admins": $ENFORCE_ADMINS,
  "required_pull_request_reviews": {
    "required_approving_review_count": $REVIEWS,
    "dismiss_stale_reviews": true,
    "require_code_owner_reviews": false
  },
  "restrictions": null,
  "required_linear_history": true,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "required_conversation_resolution": true
}
EOF

step "security: private vulnerability reporting, Dependabot alerts and security updates, secret scanning"
gh api --method PUT "repos/$REPO/private-vulnerability-reporting" --silent
gh api --method PUT "repos/$REPO/vulnerability-alerts" --silent
gh api --method PUT "repos/$REPO/automated-security-fixes" --silent
gh api --method PATCH "repos/$REPO" --silent --input - <<EOF
{
  "security_and_analysis": {
    "secret_scanning": { "status": "enabled" },
    "secret_scanning_push_protection": { "status": "enabled" }
  }
}
EOF

step "done"
echo "Secrets to add by hand (docs/src/contributing/releasing.md says how to make each one):"
secrets=$(gh secret list --repo "$REPO" --json name --jq '.[].name' 2>/dev/null || true)
for secret in CLOUDFLARE_API_TOKEN CLOUDFLARE_ACCOUNT_ID HOMEBREW_TAP_TOKEN RELEASE_TOKEN; do
    if printf '%s\n' "$secrets" | grep -qx "$secret"; then
        echo "  $secret: set"
    else
        echo "  $secret: not set (gh secret set $secret --repo $REPO)"
    fi
done
