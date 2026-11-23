"""Helpers shared by the CI scripts in this directory."""
import os
import subprocess
import sys


def fail(message):
    print(f"::error::{message}")
    sys.exit(1)


def git(*args):
    result = subprocess.run(["git", *args], capture_output=True, text=True)
    if result.returncode != 0:
        fail(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def set_outputs(values):
    """Print `key=value` lines and append them to $GITHUB_OUTPUT when running in Actions."""
    lines = "".join(f"{key}={value}\n" for key, value in values.items())
    print(lines, end="")
    if "GITHUB_OUTPUT" in os.environ:
        with open(os.environ["GITHUB_OUTPUT"], "a") as f:
            f.write(lines)


def tag_exists(tag):
    """Whether origin has the tag `tag`.

    The remote is asked directly: actions/checkout fetches no tags, and a local tag may be stale.
    With --exit-code, git exits with 2 when it reached the remote and found no such tag; any
    other failure means the answer is unknown, so it fails the run instead of guessing.
    """
    ref = f"refs/tags/{tag}"
    result = subprocess.run(
        ["git", "ls-remote", "--exit-code", "--tags", "origin", ref], capture_output=True, text=True
    )
    if result.returncode not in (0, 2):
        fail(f"git ls-remote origin {ref} failed: {result.stderr.strip()}")
    return result.returncode == 0


def last_commit(path, *pickaxe):
    """Return the latest commit on the mainline whose change to `path` matches `pickaxe`, or "".

    `pickaxe` is git log's -S or -G with its argument. With --first-parent, a merge commit counts
    as making the change its branch made, e.g. GitHub's merge commit for a PR.
    """
    return git("log", "-1", "--first-parent", "--format=%H", *pickaxe, "--", path).strip()


def is_release_run(commit):
    """Whether this run releases the version that `commit` cut.

    Only the run on that commit itself and a manual one do. Any later push or PR doesn't, so a
    failed release is finished on purpose, never as a side effect of merging something else.
    """
    manual = os.environ.get("GITHUB_EVENT_NAME") == "workflow_dispatch"
    return manual or commit == git("rev-parse", "HEAD").strip()
