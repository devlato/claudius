#!/bin/sh
# Commits the working tree's changes to a branch through GitHub's API, which signs the commit: main takes only signed
# commits, and GitHub will not merge a pull request that has an unsigned one. The branch is made at HEAD, or moved
# back to it when it is left over from an earlier run.
#
#   scripts/ci/commit-via-api.sh <branch> <message>
#
# Needs gh logged in with a token that may write the repository's contents (GH_TOKEN), jq 1.6 or newer, and
# GITHUB_REPOSITORY (owner/name). HEAD has to be on GitHub already. The changed files must be text (the manifests,
# Cargo.lock and CHANGELOG.md), and none may be deleted.

set -eu

branch=$1
message=$2
repo=${GITHUB_REPOSITORY:?set GITHUB_REPOSITORY to owner/name}
head=$(git rev-parse HEAD)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

if [ -n "$(git diff --name-only --diff-filter=D)" ]; then
    echo "commit-via-api: files were deleted, and this script only adds and changes files" >&2
    exit 1
fi
{
    git diff --name-only
    git ls-files --others --exclude-standard
} | sort -u > "$tmp/files"
if [ ! -s "$tmp/files" ]; then
    echo "commit-via-api: nothing to commit" >&2
    exit 1
fi

if gh api "repos/$repo/git/ref/heads/$branch" --silent 2>/dev/null; then
    gh api --method PATCH "repos/$repo/git/refs/heads/$branch" -f sha="$head" -F force=true --silent
else
    gh api --method POST "repos/$repo/git/refs" -f ref="refs/heads/$branch" -f sha="$head" --silent
fi

# jq reads each file itself: a Cargo.lock is too long to pass as one command-line argument
while IFS= read -r file; do
    jq -n --arg path "$file" --rawfile text "$file" '{ path: $path, contents: ($text | @base64) }'
done < "$tmp/files" | jq -s . > "$tmp/additions.json"
jq -n --arg repo "$repo" --arg branch "$branch" --arg head "$head" --arg message "$message" \
    --slurpfile additions "$tmp/additions.json" '{
  query: "mutation($input: CreateCommitOnBranchInput!) { createCommitOnBranch(input: $input) { commit { url } } }",
  variables: { input: {
    branch: { repositoryNameWithOwner: $repo, branchName: $branch },
    expectedHeadOid: $head,
    message: { headline: $message },
    fileChanges: { additions: $additions[0] }
  } }
}' > "$tmp/request.json"
gh api graphql --input "$tmp/request.json" --jq '.data.createCommitOnBranch.commit.url'
