#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (c) 2026 Florian Harr. Added October 10, 2026.
"""Propose upstream main and release merges without changing the deployed branch."""
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def main():
    upstream = 'https://github.com/rustmailer/bichon.git'
    repo = os.environ['GITHUB_REPOSITORY']
    release = json.loads(run('gh', 'api', 'repos/rustmailer/bichon/releases/latest'))['tag_name']
    candidates = []
    run('git', 'config', 'user.name', 'github-actions[bot]')
    run('git', 'config', 'user.email', '41898282+github-actions[bot]@users.noreply.github.com')
    run('git', 'fetch', 'origin', 'main')
    for kind, ref in (('main', 'refs/heads/main'), ('release', 'refs/tags/' + release)):
        run('git', 'fetch', upstream, ref)
        target = run('git', 'rev-parse', 'FETCH_HEAD^{commit}')
        if not re.fullmatch(r'[0-9a-f]{40}', target):
            raise RuntimeError('Upstream did not resolve to a commit SHA')
        contained = subprocess.run(['git', 'merge-base', '--is-ancestor', target, 'origin/main'])
        if contained.returncode == 0:
            continue
        if contained.returncode != 1:
            raise RuntimeError('Cannot compare upstream history')
        branch = 'automation/upstream-' + kind
        run('git', 'checkout', '-B', branch, 'origin/main')
        merged = subprocess.run(['git', 'merge', '--no-edit', target])
        if merged.returncode:
            conflicts = run('git', 'diff', '--name-only', '--diff-filter=U')
            run('git', 'merge', '--abort')
            raise RuntimeError(f'Upstream {kind} requires manual conflict resolution:\n{conflicts}')
        run('git', 'push', '--force-with-lease', 'origin', 'HEAD:refs/heads/' + branch)
        existing = json.loads(run('gh', 'pr', 'list', '--repo', repo, '--head', branch, '--base', 'main', '--json', 'number'))
        body = f'''## Summary of changes

Upstream {kind} moved to `{target}`. This candidate merges it into the maintained fork so we can check attachment fidelity and OIDC before updating the base.

The scheduled workflow runs the fork checks with read-only permissions and records `fork/upstream-validation` on this commit. The PR stays a draft for review. It doesn't deploy or replace mail data.
'''
        with tempfile.NamedTemporaryFile(mode='w', suffix='.md') as body_file:
            body_file.write(body)
            body_file.flush()
            if existing:
                run('gh', 'pr', 'edit', str(existing[0]['number']), '--repo', repo, '--body-file', body_file.name)
            else:
                run('gh', 'pr', 'create', '--repo', repo, '--base', 'main', '--head', branch, '--draft', '--title', 'chore(upstream): merge latest ' + kind, '--body-file', body_file.name)
        candidates.append({'branch': branch, 'sha': run('git', 'rev-parse', 'HEAD')})
    with Path(os.environ['GITHUB_OUTPUT']).open('a') as output:
        output.write('candidates=' + json.dumps(candidates) + '\n')


if __name__ == '__main__':
    main()
