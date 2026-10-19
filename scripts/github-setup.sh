#!/bin/sh
# Repository settings as code: merging, Actions, security features, rulesets for main and the release tags, the
# environments that hold the release and deploy secrets, and the topics. Every call sets a value rather than adding one, so running it again is safe. It needs `gh` logged in as an
# admin of the repository. Features GitHub Free offers only to public repositories are skipped, with a message, while
# the repository is private; run it again once it is public.
#
#   scripts/github-setup.sh
#   REVIEWS=1 scripts/github-setup.sh       # once there is a second maintainer to approve pull requests
#   RELEASE_REVIEWERS=devlato scripts/github-setup.sh    # a person approves every release before it is published
#
# Environment:
#   REPO            owner/name (default: devlato/claudius)
#   REVIEWS         approving reviews a pull request needs (default: 0, for a single maintainer; a pull request is
#                   still required, and nobody can approve their own)
#   RELEASE_REVIEWERS  GitHub logins, separated by spaces, one of whom approves each release in the environment
#                   "release" before it is published (default: none, so that merging a release PR releases)
#   HOMEPAGE        the website, set as the repository's homepage when given
#   DESCRIPTION     the repository's description, set when given (otherwise the one on GitHub stays)

set -eu

REPO=${REPO:-devlato/claudius}
REVIEWS=${REVIEWS:-0}
RELEASE_REVIEWERS=${RELEASE_REVIEWERS:-}
HOMEPAGE=${HOMEPAGE:-}
DESCRIPTION=${DESCRIPTION:-}
TOPICS="music music-production beatmaker audio rust egui wasm mcp strudel rhai"
# the GitHub Actions app, which runs the workflows: it alone reports "ci ok"
ACTIONS_APP=15368

step() {
    printf '\n== %s\n' "$*"
}

gh auth status >/dev/null 2>&1 || {
    echo "log in first: gh auth login" >&2
    exit 1
}
VISIBILITY=$(gh api "repos/$REPO" --jq .visibility)
echo "configuring $REPO ($VISIBILITY)"

step "merging: squash only, the PR title as the commit title, branches deleted after merging, auto-merge"
set --
if [ -n "$DESCRIPTION" ]; then set -- "$@" -f description="$DESCRIPTION"; fi
if [ -n "$HOMEPAGE" ]; then set -- "$@" -f homepage="$HOMEPAGE"; fi
gh api --method PATCH "repos/$REPO" --silent "$@" \
    -F has_wiki=false \
    -F has_projects=false \
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

step "dependencies and releases: Dependabot alerts and security updates; a published release cannot change"
gh api --method PUT "repos/$REPO/vulnerability-alerts" --silent
gh api --method PUT "repos/$REPO/automated-security-fixes" --silent
gh api --method PUT "repos/$REPO/immutable-releases" --silent

step "actions: GitHub's own and the few others the workflows use, each pinned to a commit; a read-only token"
gh api --method PUT "repos/$REPO/actions/permissions" --silent --input - <<'EOF'
{ "enabled": true, "allowed_actions": "selected", "sha_pinning_required": true }
EOF
# exactly the third-party actions in .github/: adding one to a workflow means adding it here too
gh api --method PUT "repos/$REPO/actions/permissions/selected-actions" --silent --input - <<'EOF'
{
  "github_owned_allowed": true,
  "verified_allowed": false,
  "patterns_allowed": [
    "Swatinem/rust-cache@*",
    "cloudflare/wrangler-action@*",
    "crate-ci/typos@*",
    "ossf/scorecard-action@*",
    "taiki-e/install-action@*"
  ]
}
EOF
# The one setting lets workflows both open pull requests (release-pr.yml does, with its own token, when there is no
# RELEASE_TOKEN) and approve them. Once approvals count, a workflow must not give one, so it is off, and release
# PRs need RELEASE_TOKEN.
if [ "$REVIEWS" -gt 0 ]; then workflows_open_prs=false; else workflows_open_prs=true; fi
gh api --method PUT "repos/$REPO/actions/permissions/workflow" --silent \
    -f default_workflow_permissions=read \
    -F can_approve_pull_request_reviews=$workflows_open_prs
# logs and artifacts go after a week, so anything a run printed by mistake does not linger
gh api --method PUT "repos/$REPO/actions/permissions/artifact-and-log-retention" --silent -F days=7
# codeql.yml and scorecard.yml run only on a public repository
gh variable set REPO_IS_PUBLIC --repo "$REPO" --body "$([ "$VISIBILITY" = public ] && echo true || echo false)"

if [ "$VISIBILITY" != public ]; then
    # while private: no other repository may use this one's workflows, and pull requests from forks run nothing
    gh api --method PUT "repos/$REPO/actions/permissions/access" --silent -f access_level=none
    printf '\n%s\n%s\n' "Private on GitHub Free: rulesets (so the protection of main and the tags), environments, secret scanning" \
        "and private vulnerability reporting need a public repository or a paid plan. Run this again once it is public."
    exit 0
fi

step "security: secret scanning with push protection, private vulnerability reporting"
gh api --method PATCH "repos/$REPO" --silent --input - <<'EOF'
{
  "security_and_analysis": {
    "secret_scanning": { "status": "enabled" },
    "secret_scanning_push_protection": { "status": "enabled" }
  }
}
EOF
gh api --method PUT "repos/$REPO/private-vulnerability-reporting" --silent
# a pull request from a fork runs no workflow until a maintainer approves it, for anyone outside the repository
gh api --method PUT "repos/$REPO/actions/permissions/fork-pr-contributor-approval" --silent \
    -f approval_policy=all_external_contributors

# a ruleset is updated in place when it exists, so that running this again never makes a second one
upsert_ruleset() {
    id=$(gh api "repos/$REPO/rulesets" --jq ".[] | select(.name == \"$1\") | .id")
    if [ -n "$id" ]; then
        gh api --method PUT "repos/$REPO/rulesets/$id" --silent --input -
    else
        gh api --method POST "repos/$REPO/rulesets" --silent --input -
    fi
}

step "main: pull requests only ($REVIEWS approving reviews), \"ci ok\" on an up-to-date branch, signed linear history"
# no bypass, admins included, so that an admin's token, or an agent using it, cannot push to main either
upsert_ruleset main <<EOF
{
  "name": "main",
  "target": "branch",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] } },
  "bypass_actors": [],
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    { "type": "required_linear_history" },
    { "type": "required_signatures" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": $REVIEWS,
        "dismiss_stale_reviews_on_push": true,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": true
      }
    },
    {
      "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": true,
        "do_not_enforce_on_create": false,
        "required_status_checks": [{ "context": "ci ok", "integration_id": $ACTIONS_APP }]
      }
    }
  ]
}
EOF

# Creating a tag stays open: the release workflow makes the tag when it publishes, and a repository that belongs to a
# person, not an organisation, cannot let the GitHub Actions app bypass a ruleset. A published release and its tag
# cannot change anyway (immutable releases).
step "release tags: v* tags are never moved or deleted (an admin can, to repair a failed release)"
upsert_ruleset "release tags" <<EOF
{
  "name": "release tags",
  "target": "tag",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["refs/tags/v*"], "exclude": [] } },
  "bypass_actors": [{ "actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always" }],
  "rules": [{ "type": "update" }, { "type": "deletion" }]
}
EOF

# An environment's secrets reach only the jobs that name it, and only on the branches it lists: a workflow on another
# branch cannot read them, nor a pull request. release: publish and homebrew in release.yml, from main or a hotfix
# branch. release-pr: release-pr.yml. site: the deploy job in site.yml.
upsert_environment() {
    name=$1
    reviewers=$2
    shift 2
    gh api --method PUT "repos/$REPO/environments/$name" --silent --input - <<EOF
{
  "wait_timer": 0,
  "prevent_self_review": false,
  "reviewers": [$reviewers],
  "deployment_branch_policy": { "protected_branches": false, "custom_branch_policies": true }
}
EOF
    policies=$(gh api "repos/$REPO/environments/$name/deployment-branch-policies" --jq '.branch_policies[].name')
    for branch in "$@"; do
        if ! printf '%s\n' "$policies" | grep -qxF "$branch"; then
            gh api --method POST "repos/$REPO/environments/$name/deployment-branch-policies" --silent \
                -f name="$branch" -f type=branch
        fi
    done
}

reviewers=
for login in $RELEASE_REVIEWERS; do
    id=$(gh api "users/$login" --jq .id)
    reviewers="${reviewers:+$reviewers, }{ \"type\": \"User\", \"id\": $id }"
done
step "environments: release (approved by: ${RELEASE_REVIEWERS:-nobody, merging a release PR releases}), release-pr, site"
upsert_environment release "$reviewers" main "hotfix/v*"
upsert_environment release-pr "" main
upsert_environment site "" main

step "hotfix branches: only admins create and push hotfix/* (release.yml releases from them)"
upsert_ruleset "hotfix branches" <<EOF
{
  "name": "hotfix branches",
  "target": "branch",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["refs/heads/hotfix/*"], "exclude": [] } },
  "bypass_actors": [{ "actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always" }],
  "rules": [{ "type": "creation" }, { "type": "update" }, { "type": "deletion" }, { "type": "non_fast_forward" }]
}
EOF

step "done"
echo "Secrets to add by hand (docs/src/contributing/releasing.md says how to make each one):"
for entry in site/CLOUDFLARE_API_TOKEN site/CLOUDFLARE_ACCOUNT_ID release/HOMEBREW_TAP_TOKEN release-pr/RELEASE_TOKEN; do
    environment=${entry%%/*}
    secret=${entry#*/}
    if gh secret list --repo "$REPO" --env "$environment" --json name --jq '.[].name' | grep -qx "$secret"; then
        echo "  $secret ($environment): set"
    else
        echo "  $secret ($environment): not set (gh secret set $secret --repo $REPO --env $environment)"
    fi
done
echo "The variable for the site, which deploys once it is set:"
if gh variable list --repo "$REPO" --json name --jq '.[].name' | grep -qx SITE_URL; then
    echo "  SITE_URL: set"
else
    echo "  SITE_URL: not set (gh variable set SITE_URL --repo $REPO --body https://claudius.devlato.com)"
fi
