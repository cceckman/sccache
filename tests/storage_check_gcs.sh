#!/bin/sh

# Repro case for #2132.
# Not itself reproducible - this uses cceckman's own creds.

set -eux

export SCCACHE_GCS_KEY_PATH=/tmp/sccache-gcs.json
export SCCACHE_GCS_BUCKET=sccache-dev.cceckman.com

# gcloud storage rm -r "gs://$SCCACHE_GCS_BUCKET/*"

# Use sccache for the build itself,
# so we test the above settings:
export SCCACHE_GCS_RW_MODE=READ_WRITE
export RUSTC_WRAPPER=$(which sccache)

# Skip the check where possible, from PR #2133:
export SCCACHE_ASSUME_RW_MODE=READ_WRITE

cargo test --no-run

# The same settings apply for the test
cargo test --test storage_check