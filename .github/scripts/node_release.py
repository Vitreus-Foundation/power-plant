#!/usr/bin/env python3
"""Steps of the node release workflow, .github/workflows/node-release.yml. Run from the
repository root:

    python3 .github/scripts/node_release.py should-release
    python3 .github/scripts/node_release.py check-image IMAGE VERSION COMMIT

The node's version is `version` in [workspace.package] of Cargo.toml. Release tags are looked up
on the `origin` remote.
"""
import argparse
import subprocess
import tomllib

from workflow import fail, is_release_run, last_commit, set_outputs, tag_exists

MANIFEST = "Cargo.toml"

# Every released version gets a tag, e.g. node-v1.1.1; the tag tells whether it was released.
TAG = "node-v{}"

# The binaries in the image, relative to its working directory.
NODE = "./target/release/vitreus-power-plant-node"
WORKERS = ["./target/release/polkadot-prepare-worker", "./target/release/polkadot-execute-worker"]


def release_commit(version):
    """Return the commit that set the node version to `version`: the release is built from it."""
    # -S counts the occurrences, so a later edit of the line that keeps the value, such as a
    # comment after it, doesn't count. The newline in front skips the dependencies, which give
    # their version inside braces; the node's own package takes the workspace one.
    commit = last_commit(MANIFEST, "-S", f'\nversion = "{version}"')
    if not commit:
        fail(f"no commit sets version {version} in {MANIFEST}")
    return commit


def should_release():
    """Decide whether this run releases the node, and print what the publish job needs for that.

    Without tag node-vX.Y.Z, version X.Y.Z gets released from the commit that set it, in a run
    that `is_release_run` allows: the merge of the PR that bumps it, or a manual one. Any other
    run only gets a warning. A push that changes Cargo.toml but not the version finds the tag and
    does nothing.
    """
    with open(MANIFEST, "rb") as f:
        version = tomllib.load(f)["workspace"]["package"]["version"]
    tag = TAG.format(version)
    release, commit = 0, ""
    if not tag_exists(tag):
        commit = release_commit(version)
        if is_release_run(commit):
            release = 1
        else:
            print(
                f"::warning::node {version} isn't released yet (no {tag}), and this run doesn't "
                f"publish it. If its release run failed, run Node release on main by hand"
            )
            commit = ""
    set_outputs({"should-release": release, "version": version, "tag": tag, "commit": commit})


def run_in(image, *command):
    """Return what `command` prints in a container of `image`, or fail the run if it fails."""
    # Without a local image, docker would pull the published one and check that instead.
    docker = ["docker", "run", "--rm", "--pull=never", image, *command]
    result = subprocess.run(docker, capture_output=True, text=True)
    if result.returncode != 0:
        fail(f"{' '.join(command)} fails in {image}: {result.stderr.strip()}")
    return result.stdout.strip()


def check_image(image, version, commit):
    """Fail unless the binaries start in `image` and the node reports `version` and `commit`."""
    # The binaries are built in the rust image but run in the ubuntu one, which may lack what they
    # link against, such as a newer glibc. The node reports the version of the cli crate, which
    # differs if that crate gets a version of its own, and the commit it was built from, which
    # becomes `unknown` if the build loses .git.
    expected = f"vitreus-power-plant-node {version}-{commit[:11]}-"
    reported = run_in(image, NODE, "--version")
    if not reported.startswith(expected):
        fail(f"the image reports {reported!r}, expected {expected!r}…")
    for worker in WORKERS:
        run_in(image, worker, "--version")
    print(f"{image} runs node {version} from {commit[:11]}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Steps of the node release workflow.")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("should-release", help="decide whether this run releases the node, print what publishing needs")
    check_parser = commands.add_parser("check-image", help="check that the image runs this version and commit")
    check_parser.add_argument("image")
    check_parser.add_argument("version")
    check_parser.add_argument("commit")
    args = parser.parse_args()
    if args.command == "should-release":
        should_release()
    else:
        check_image(args.image, args.version, args.commit)
