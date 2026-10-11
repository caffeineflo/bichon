"""Exercise candidate updates with real local Git histories and no GitHub calls."""
import contextlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('sync_upstream', Path(__file__).parents[1] / 'sync-upstream.py')
sync = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sync)


class UpstreamCandidatesTest(unittest.TestCase):
    def test_conflict_does_not_block_release_or_rebuild_unchanged_candidate(self):
        with patch.dict(os.environ, {'GIT_CONFIG_GLOBAL': os.devnull, 'GIT_CONFIG_NOSYSTEM': '1'}), tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            upstream, origin, fork = [root / name for name in ('upstream', 'origin.git', 'fork')]

            def git(cwd, *args):
                return subprocess.check_output(['git', '-C', str(cwd), *args], text=True, stderr=subprocess.DEVNULL).strip()

            upstream.mkdir()
            git(upstream, 'init', '-b', 'main')
            git(upstream, 'config', 'user.name', 'Fixture')
            git(upstream, 'config', 'user.email', 'fixture@example.invalid')
            (upstream / 'shared').write_text('base\n')
            git(upstream, 'add', '.')
            git(upstream, 'commit', '-m', 'base')
            base = git(upstream, 'rev-parse', 'HEAD')
            git(upstream, 'checkout', '-b', 'release')
            (upstream / 'release').write_text('release\n')
            git(upstream, 'add', '.')
            git(upstream, 'commit', '-m', 'release')
            git(upstream, 'tag', 'v1')
            git(upstream, 'checkout', 'main')
            (upstream / 'shared').write_text('upstream\n')
            git(upstream, 'commit', '-am', 'upstream change')
            origin.mkdir()
            git(origin, 'init', '--bare')
            git(root, 'clone', str(upstream), str(fork))
            git(fork, 'remote', 'set-url', 'origin', str(origin))
            git(fork, 'reset', '--hard', base)
            git(fork, 'config', 'user.name', 'Fixture')
            git(fork, 'config', 'user.email', 'fixture@example.invalid')
            (fork / 'shared').write_text('fork\n')
            git(fork, 'commit', '-am', 'fork change')
            git(fork, 'push', 'origin', 'main')
            original_run = sync.run
            prs = set()

            def local_run(*args):
                if args[:3] == ('git', 'fetch', 'https://github.com/rustmailer/bichon.git'):
                    return original_run('git', 'fetch', str(upstream), args[3])
                if args[:2] == ('gh', 'api'):
                    return json.dumps({'tag_name': 'v1'})
                if args[:3] == ('gh', 'pr', 'list'):
                    branch = args[args.index('--head') + 1]
                    return json.dumps([{'number': 1}] if branch in prs else [])
                if args[:3] == ('gh', 'pr', 'create'):
                    prs.add(args[args.index('--head') + 1])
                    return 'https://example.invalid/pr/1'
                if args[:3] == ('gh', 'pr', 'edit'):
                    return ''
                return original_run(*args)

            results = []
            with contextlib.chdir(fork), patch.object(sync, 'run', side_effect=local_run):
                for day in (1, 2):
                    output = root / f'output-{day}'
                    with patch.dict(os.environ, {
                        'GITHUB_REPOSITORY': 'fixture/fork', 'GITHUB_OUTPUT': str(output),
                        'GIT_AUTHOR_DATE': f'2026-10-0{day}T00:00:00Z',
                        'GIT_COMMITTER_DATE': f'2026-10-0{day}T00:00:00Z',
                    }):
                        sync.main()
                    results.append({key: json.loads(value) for key, value in
                                    (line.split('=', 1) for line in output.read_text().splitlines())})
            self.assertEqual(results[0]['conflicts'], [{'kind': 'main', 'files': ['shared']}])
            self.assertEqual([c['branch'] for c in results[0]['candidates']], ['automation/upstream-release'])
            self.assertEqual(results[1]['candidates'], results[0]['candidates'])


if __name__ == '__main__':
    unittest.main()
