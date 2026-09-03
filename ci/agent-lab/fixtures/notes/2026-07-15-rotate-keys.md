suggested-domain: ops

Rotate the deploy keys every quarter. New key: `openssl rand -hex 32`, paste
it into the vault under deploy/runner, then restart the runner so it picks
the new one up. The old key stays valid until the restart, not longer.
