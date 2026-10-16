#!/usr/bin/env bash
#
# Checks that every commit in BASE..HEAD carries a "Signed-off-by:" line
# matching its author, as required by the Developer Certificate of Origin
# (https://developercertificate.org/). See CONTRIBUTING.md.
#
#     ./scripts/check-dco.sh origin/main HEAD

set -euo pipefail

base="${1:?usage: check-dco.sh BASE HEAD}"
head="${2:?usage: check-dco.sh BASE HEAD}"
failed=0

for commit in $(git rev-list --no-merges "$base..$head"); do
    author="$(git show -s --format='%an <%ae>' "$commit")"
    if ! git show -s --format='%B' "$commit" | grep -qiF "Signed-off-by: $author"; then
        echo "::error::$(git show -s --format='%h %s' "$commit") is not signed off by $author"
        failed=1
    fi
done

if [ "$failed" -ne 0 ]; then
    echo "Sign off with 'git commit -s'. To fix existing commits: git rebase --signoff $base"
    exit 1
fi
echo "All commits are signed off."
