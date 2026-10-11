# Bichon - temporary OIDC fork

We use and have tested this fork's OIDC integration with [Pocket ID](https://github.com/pocket-id/pocket-id), including passkey sign-in. We intend to keep it current with [upstream Bichon](https://github.com/rustmailer/bichon) until Bichon Pro or another official upstream release provides OIDC that works with Pocket ID. At that point, we plan to move back to the official release.

Bichon is a great app, and its author deserves support. You can [support the author directly](https://buymeacoffee.com/rustmailer) and choose the official paid edition when it's available and meets your needs. See the [README](README.md#why-this-temporary-fork-exists) for the purpose of this fork.

This fork adds native OIDC and fixes lossless attachment reconstruction on Bichon 2.1.1. We added these changes on October 10, 2026. The source remains available under the upstream AGPL license, and the UI links to the deployed source revision through `/api/source`.

## OIDC configuration

The `bichon-oidc` crate uses the standard OpenID Connect protocol and has no dependencies on Bichon's user database or mail storage. The adapter lives in `crates/server/src/rest/public/oidc.rs`; its public routes and sign-in UI are the remaining integration points when merging upstream changes.

Set these environment variables on the server:

```text
BICHON_OIDC_ISSUER=https://your-provider.example
BICHON_OIDC_CLIENT_ID=your-client-id
BICHON_OIDC_CLIENT_SECRET=your-confidential-client-secret
BICHON_OIDC_REDIRECT_URL=https://archive.example/api/auth/oidc/callback
BICHON_OIDC_USER_MAPPING={"stable-provider-subject":"existing-bichon-username"}
```

Register the exact redirect URL with your provider. For a base path such as `/archive`, include it in both Bichon's base URL and the redirect URL. Public clients can omit the client secret. Keep secrets out of Compose source and use a protected `.env` file or your deployment's secret store.

The flow uses authorization code with PKCE S256, state, nonce, HTTPS discovery, and signed ID token validation. Provider keys refresh on each flow. Only explicitly mapped subjects can sign in, and their local Bichon user must already exist. We don't link users by email or create administrators automatically. Local login remains available, and an enabled local TOTP requirement still applies after OIDC.

Sign-in state expires after five minutes. The browser-bound session handoff is one-use and expires after one minute; no Bichon bearer token appears in a redirect URL. These transient flows live in memory, so a restart requires a new sign-in and this adapter assumes one server instance.

Use your existing IP restriction when moving from TinyAuth to native OIDC. A protected mail API must still reject unauthenticated requests. Pocket ID client group restrictions provide a second access boundary in addition to the exact subject mapping.

## Attachment recovery

The patch stores encoded MIME bodies separately from decoded file identity. Raw storage keys use a separate hash domain to prevent collisions with legacy blobs or original email keys. Reconstruction replaces each placeholder once, and deletion reference checks follow the actual stored blob key.

Legacy metadata remains readable, but the patch can't recover bytes that an older version overwrote. Reimport affected archives from original MBOX/EML files into isolated fresh mail indices and blob storage, then compare every downloaded message and decoded MIME payload with its source before replacing the active archive. Preserve a rollback copy of the original data and image. Don't delete source mail during this upgrade.

The focused upstream fix is https://github.com/rustmailer/bichon/pull/375.

## Build and verification

Use Rust 1.95.0, Node 24, and pnpm 10.23.0. `python3 scripts/verify-fork.py` runs the affected attachment, metadata, OIDC, server adapter, and login checks. The upstream full suite includes external IMAP accounts and non-portable fixtures; a passing focused run doesn't claim that entire suite passed.

Build the container with `docker build -f docker/Dockerfile.fork --build-arg SOURCE_REVISION=<full-commit-sha> -t bichon-fork:<full-commit-sha> .`. The multi-stage build compiles the UI from the same source and embeds it in the Rust server. Main branch CI publishes `ghcr.io/caffeineflo/bichon:<full-commit-sha>`. We retain that image path after renaming the repository to `bichon-temporary-oidc` so existing deployments keep working. Pin deployment to the image digest after archive verification.

## Upstream updates

The daily `Propose upstream updates` workflow checks upstream `main` and the latest release. Each moved ref gets a separate merge candidate PR. A conflict in one candidate doesn't block the other; the final report fails the run with the files needing resolution. Existing candidate commits stay unchanged until upstream or the fork base moves. The workflow leaves `main` and deployed data untouched.

Candidate verification runs in a separate job without write credentials. Because GitHub's workflow token doesn't trigger another workflow when it creates a PR, this workflow records its own `fork/upstream-validation` status on the candidate commit. A failure in either candidate conservatively marks both candidates failed. The PRs stay drafts for review.

Enable Actions and "Allow GitHub Actions to create and approve pull requests" in the fork's repository settings. Review candidate changes and rerun full archive fidelity checks before merging an update that touches MIME extraction, storage, or export. Merge after review to publish a new image; deploy it explicitly with a rollback backup. Don't make manual edits on the automation branches, which the workflow replaces using a lease.
