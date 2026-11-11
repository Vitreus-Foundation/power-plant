#!/usr/bin/env python3
"""Dependency checks for CI. Run from the repository root:

    python3 .github/scripts/dependencies.py sdk-pin
"""
import argparse
import tomllib
from urllib.parse import urlsplit

from changelog import fail, git

MANIFEST = "Cargo.toml"
LOCK = "Cargo.lock"
SDK = "https://github.com/paritytech/polkadot-sdk"


def repository(url):
    """Return the host and path that identify the repository of a git URL."""
    # Cargo.lock spells a source the way the manifest that brought it in does, so one repository
    # can come with or without `.git` or a trailing slash, in another case, or over SSH.
    parts = urlsplit(url)
    return parts.hostname, parts.path.rstrip("/").removesuffix(".git").lower()


def sdk_tag_commit(tag):
    """Return the commit of the polkadot-sdk tag `tag`, or None without such a tag."""
    ref = f"refs/tags/{tag}"
    # ls-remote lists an annotated tag as a tag object. The commit it points to comes on its own
    # `^{}` line, and only when that line is asked for too.
    refs = {}
    for line in git("ls-remote", "--tags", SDK, ref, f"{ref}^{{}}").splitlines():
        sha, name = line.split("\t")
        refs[name] = sha
    return refs.get(f"{ref}^{{}}", refs.get(ref))


def sdk_pin():
    """Fail unless the Polkadot SDK in Cargo.lock is on the tag that Cargo.toml names.

    The tag is in [workspace.metadata.polkadot-sdk]. The comment on that table says why and how
    to put the lock back.
    """
    with open(MANIFEST, "rb") as f:
        tag = tomllib.load(f)["workspace"]["metadata"]["polkadot-sdk"]["tag"]
    with open(LOCK, "rb") as f:
        packages = tomllib.load(f)["package"]
    sdk = repository(SDK)
    # A Cargo.lock source is `<kind>+<url>`.
    sources = {
        p["source"] for p in packages if repository(p.get("source", "").partition("+")[2]) == sdk
    }
    commits = sorted({source.rpartition("#")[2] for source in sources})
    if len(commits) != 1:
        fail(f"expected one polkadot-sdk commit in {LOCK}, found {', '.join(commits) or 'none'}")
    commit = commits[0]
    tag_commit = sdk_tag_commit(tag)
    if tag_commit is None:
        fail(f"polkadot-sdk has no tag {tag}, the one {MANIFEST} names")
    if commit != tag_commit:
        fail(
            f"polkadot-sdk in {LOCK} is at {commit}, not at {tag}: set its tag in {MANIFEST} "
            f"if the SDK moved on purpose, or put the lock back with "
            f"`cargo update -p frame-support --precise refs/tags/{tag}`"
        )
    print(f"polkadot-sdk in {LOCK} is at {tag}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Dependency checks for CI.")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("sdk-pin", help="fail unless the Polkadot SDK in Cargo.lock is on the tag Cargo.toml names")
    args = parser.parse_args()
    {"sdk-pin": sdk_pin}[args.command]()
