#!/usr/bin/env python3
"""CHANGELOG.md checks for CI. Run from the repository root:

    python3 .github/scripts/changelog.py check [--base REV]
    python3 .github/scripts/changelog.py updated --base REV

In CI, --base is HEAD^1: the first parent of GitHub's merge commit for the PR, which is the
base branch. Locally, pass the fork point instead: --base "$(git merge-base origin/develop HEAD)".
release.py builds on this module.
"""
import argparse
import re
import subprocess
import sys

CHANGELOG = "CHANGELOG.md"
RUNTIME = "runtime/vitreus/src/lib.rs"

UNRELEASED = "## [Unreleased]"
RELEASE = re.compile(r"## \[([0-9]+)\] - [0-9]{4}-[0-9]{2}-[0-9]{2}")
# Subsections allowed under a version, the same set as in polkadot-fellows/runtimes.
SUBSECTIONS = {"Added", "Changed", "Fixed", "Removed"}


def fail(message):
    print(f"::error::{message}")
    sys.exit(1)


def git(*args):
    result = subprocess.run(["git", *args], capture_output=True, text=True)
    if result.returncode != 0:
        fail(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def spec_version(source):
    # Every `spec_version:` line in lib.rs, e.g. one per network if VERSION is ever split,
    # must carry the same value.
    found = {v.replace("_", "") for v in re.findall(r"^\s*spec_version: ([0-9_]+),", source, re.M)}
    if len(found) != 1:
        fail(f"expected one spec_version value in {RUNTIME}, found {sorted(found) or 'none'}")
    return found.pop()


def parse(text):
    """Return the version sections, top first, and the format errors as (line, message)."""
    versions, errors, subsections = [], [], set()
    last_release = None
    for number, line in enumerate(text.splitlines(), 1):
        if line.startswith("####"):
            continue
        if line.startswith("### "):
            name = line[4:].strip()
            if not versions:
                errors.append((number, f"'{line}' comes before any version section"))
            elif name not in SUBSECTIONS:
                allowed = ", ".join(sorted(SUBSECTIONS))
                errors.append((number, f"unknown subsection '{name}', allowed: {allowed}"))
            elif name in subsections:
                errors.append((number, f"subsection '{name}' repeats within [{versions[-1]}]"))
            subsections.add(name)
        elif line.startswith("## "):
            subsections = set()
            if not versions and line != UNRELEASED:
                errors.append((number, f"the first section must be '{UNRELEASED}'; keep it on top even when empty"))
            if line == UNRELEASED:
                if "Unreleased" in versions:
                    errors.append((number, f"a second '{UNRELEASED}': move its entries into the first one"))
                elif versions:
                    errors.append((number, "[Unreleased] must be the first section"))
                versions.append("Unreleased")
            elif m := RELEASE.fullmatch(line):
                release = int(m.group(1))
                if last_release is not None and release >= last_release:
                    errors.append((number, f"[{release}] must be lower than [{last_release}] above it"))
                last_release = release
                versions.append(m.group(1))
            else:
                expected = f"'{UNRELEASED}' or '## [<spec_version>] - YYYY-MM-DD'"
                errors.append((number, f"'{line}' must be {expected}"))
                versions.append(line)
        elif line.startswith("#") and line != "# Changelog":
            errors.append((number, f"'{line}': the only other header allowed is '# Changelog'"))
    if not versions:
        errors.append((1, f"no '{UNRELEASED}' section"))
    return versions, errors


def section(text, version):
    """Return the lines of the [version] section, from its header up to the next version."""
    lines, inside = [], False
    for line in text.splitlines():
        if line.startswith("## ["):
            inside = line.startswith(f"## [{version}]")
        if inside:
            lines.append(line.rstrip())
    return lines


def read_changelog():
    """Return CHANGELOG.md and its versions, or report its format errors and exit."""
    with open(CHANGELOG) as f:
        text = f.read()
    versions, errors = parse(text)
    for number, message in errors:
        print(f"::error file={CHANGELOG},line={number}::{message}")
    if errors:
        sys.exit(1)
    return text, versions


def check(base):
    """Validate CHANGELOG.md and its versions against spec_version.

    [Unreleased] always stays on top, even when empty. The release PR moves its entries into a
    new [<spec_version>] section and bumps spec_version in the same PR, so the latest version
    always equals spec_version. With --base, the PR is compared with its base: a PR that raises
    spec_version is a release and must leave [Unreleased] empty, since everything merged so far
    goes into the release; any other PR must leave the latest version's section unchanged, since
    new entries go under [Unreleased].
    """
    text, versions = read_changelog()
    released = versions[1:]
    if not released:
        return
    latest = released[0]
    with open(RUNTIME) as f:
        spec = spec_version(f.read())
    if latest != spec:
        fail(
            f"the latest version in {CHANGELOG} is [{latest}] but spec_version is {spec}; "
            f"they change together, in the release PR"
        )
    if not base:
        return
    if int(spec_version(git("show", f"{base}:{RUNTIME}"))) < int(spec):
        if any(line.strip() for line in section(text, "Unreleased")[1:]):
            fail(
                f"this PR bumps spec_version, so it is a release: move all '{UNRELEASED}' "
                f"entries into [{latest}]"
            )
    elif section(git("show", f"{base}:{CHANGELOG}"), latest) != section(text, latest):
        fail(
            f"[{latest}] was cut by an earlier PR and this PR doesn't bump spec_version: "
            f"put new entries under '{UNRELEASED}'"
        )


def updated(base):
    """The PR must change CHANGELOG.md.

    Only committed changes count: the diff is between two revisions, not the working tree.
    """
    if CHANGELOG not in git("diff", "--name-only", base, "HEAD").splitlines():
        fail(
            f"{CHANGELOG} has not been updated. Either add entries under [Unreleased] or tick "
            f"\"Does not require a CHANGELOG entry\" in the PR description"
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="CHANGELOG.md checks for CI.")
    commands = parser.add_subparsers(dest="command", required=True)
    updated_parser = commands.add_parser("updated", help="fail unless the PR changes CHANGELOG.md")
    updated_parser.add_argument("--base", required=True, help="the PR's base revision")
    check_parser = commands.add_parser("check", help="validate CHANGELOG.md against the runtime")
    check_parser.add_argument("--base", help="the PR's base revision; enables the release and released-section checks")
    args = parser.parse_args()
    {"updated": updated, "check": check}[args.command](args.base)
