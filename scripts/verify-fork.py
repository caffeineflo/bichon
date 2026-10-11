#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (c) 2026 Florian Harr. Added October 10, 2026.
"""Run the deterministic checks for the maintained fork changes."""
import os
import subprocess
import tempfile


def run(*args, cwd=None, env=None):
    subprocess.run(args, cwd=cwd, env=env, check=True)


def main():
    run('pnpm', 'install', '--frozen-lockfile', cwd='web')
    run('pnpm', 'run', 'build', cwd='web')
    run('pnpm', 'exec', 'eslint', 'src/routes/(auth)/oidc-callback.tsx', cwd='web')
    run('pnpm', 'exec', 'vitest', 'run', 'src/features/auth/__tests__/schema.test.ts', cwd='web')
    with tempfile.TemporaryDirectory(prefix='bichon-fork-check-') as data:
        env = {**os.environ, 'BICHON_ROOT_DIR': data}
        cargo = ['cargo', '+1.95.0']
        run(*cargo, 'test', '--locked', '-p', 'bichon-oidc', env=env)
        run(*cargo, 'clippy', '--locked', '-p', 'bichon-oidc', '--all-targets', '--', '-D', 'warnings', env=env)
        for test_filter in ('envelope::extractor::test', 'attachment_storage_tests', 'message::content::tests'):
            run(*cargo, 'test', '--locked', '-p', 'bichon-core', '--lib', test_filter, '--', '--test-threads=1', env=env)
        run(*cargo, 'test', '--locked', '-p', 'bichon-server', '--no-default-features', 'rest::public::oidc::tests', env=env)
        run(*cargo, 'check', '--locked', '-p', 'bichon-admin', env=env)
        run(*cargo, 'check', '--locked', '-p', 'bichon-server', env=env)


if __name__ == '__main__':
    main()
