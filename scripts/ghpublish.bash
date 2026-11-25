#!/bin/bash
set -euo pipefail

# ghpublish.bash uploads the files passed as arguments to a GitHub release:
# - a v* tag build goes to the release of that tag, created if needed, and
#   marked as a pre-release unless the tag is vX.Y.Z;
# - any other build goes to the "rolling" pre-release, replacing its assets.
#
# Usage: ./scripts/ghpublish.bash <file>...

__ref=${GITHUB_REF:-}

if [[ $__ref == "" ]]; then
	echo "FATAL: missing github ref" 1>&2
	exit 1
fi

if [[ $__ref =~ ^refs/tags/v ]]; then
	__tag=${__ref#refs/tags/}
else
	__tag=rolling
fi

prerelease="-p"
if [[ $__tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
	prerelease=""
fi

gh=${gh:-gh}

set -x

$gh release create $prerelease $__tag --target $GITHUB_SHA || true

$gh release upload $__tag --clobber "$@"
