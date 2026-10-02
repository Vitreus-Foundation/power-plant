#!/usr/bin/env python3
"""Steps of the release workflow, .github/workflows/release.yml. Run from the repository root:

    python3 .github/scripts/release.py should-release
    python3 .github/scripts/release.py collect RUNTIME VERSION WASM REPORT
    python3 .github/scripts/release.py notes VERSION REPORT... --commit SHA --out FILE

The CHANGELOG.md rules come from changelog.py. Release tags are looked up on the `origin` remote.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import tomllib

import changelog
from changelog import fail

# The build recipe apart from the network feature. srtool images are tagged <rust>-<srtool>, and
# srtool ignores rust-toolchain.toml, so the Rust part is read from there. PACKAGE and
# RUNTIME_DIR must match the srtool step in release.yml.
SRTOOL_IMAGE = "paritytech/srtool"
SRTOOL_VERSION = "0.18.2"
PROFILE = "production"
PACKAGE = "vitreus-power-plant-runtime"
RUNTIME_DIR = "runtime/vitreus"

# Every released version gets a tag, e.g. runtime-v215; the tag tells whether it was released.
TAG = "runtime-v{}"

# The release files are named after the network.
OUT = "out"
NAME = "vitreus_power_plant_{network}_runtime-v{version}"
REPORT = re.compile(r"(vitreus_power_plant_(\w+)_runtime-v[0-9]+)\.srtool\.json")


def srtool_tag():
    with open("rust-toolchain.toml", "rb") as f:
        rust = tomllib.load(f)["toolchain"]["channel"]
    return f"{rust}-{SRTOOL_VERSION}"


def set_outputs(values):
    """Print `key=value` lines and append them to $GITHUB_OUTPUT when running in Actions."""
    lines = "".join(f"{key}={value}\n" for key, value in values.items())
    print(lines, end="")
    if "GITHUB_OUTPUT" in os.environ:
        with open(os.environ["GITHUB_OUTPUT"], "a") as f:
            f.write(lines)


def tag_exists(version):
    """Whether origin has the release tag of `version`.

    The remote is asked directly: actions/checkout fetches no tags, and a local tag may be stale.
    With --exit-code, git exits with 2 when it reached the remote and found no such tag; any
    other failure means the answer is unknown, so it fails the run instead of guessing.
    """
    ref = f"refs/tags/{TAG.format(version)}"
    result = subprocess.run(
        ["git", "ls-remote", "--exit-code", "--tags", "origin", ref], capture_output=True, text=True
    )
    if result.returncode not in (0, 2):
        fail(f"git ls-remote origin {ref} failed: {result.stderr.strip()}")
    return result.returncode == 0


def released_section(version):
    """Return the [version] section as it was released, from its tag, or None without a tag."""
    if not tag_exists(version):
        return None
    # Fetch into FETCH_HEAD without creating a local tag, and keep a shallow clone shallow.
    git = changelog.git
    depth = ["--depth=1"] if git("rev-parse", "--is-shallow-repository").strip() == "true" else []
    git("fetch", "--quiet", *depth, "origin", f"refs/tags/{TAG.format(version)}")
    return changelog.section(git("show", f"FETCH_HEAD:{changelog.CHANGELOG}"), version)


def should_release():
    """Decide whether this run releases, and print what the build needs for that.

    The version is the first numbered section of CHANGELOG.md, right below [Unreleased]. Without
    tag runtime-vN it gets released, after the same checks as `changelog.py check`. With the tag
    it was released before, and its section must not have changed since: a different section
    means the number was cut again, e.g. after a revert, so the release must take the next one.
    """
    text, versions = changelog.read_changelog()
    version = versions[1] if len(versions) > 1 else ""
    release = 0
    if version:
        tagged = released_section(version)
        if tagged is None:
            changelog.check(None)
            release = 1
        elif tagged != changelog.section(text, version):
            fail(
                f"{version} is already released as {TAG.format(version)} with a different "
                f"[{version}] section; use the next free number"
            )
    set_outputs(
        {
            "should-release": release,
            "version": version,
            "tag": TAG.format(version) if version else "",
            "srtool-tag": srtool_tag(),
            "profile": PROFILE,
        }
    )


def collect(runtime, version, wasm, report):
    """Check srtool's build of `runtime`, then put its wasm and report into out/ for the release."""
    data = json.loads(report)
    spec = data["runtimes"]["compressed"]["subwasm"]["core_version"]["specVersion"]
    if str(spec) != version:
        fail(f"the {runtime} wasm has spec_version {spec}, but the release is {version}")
    name = NAME.format(network=runtime.removesuffix("-runtime"), version=version)
    os.makedirs(OUT, exist_ok=True)
    shutil.copyfile(wasm, f"{OUT}/{name}.compact.compressed.wasm")
    with open(f"{OUT}/{name}.srtool.json", "w") as f:
        json.dump(data, f, indent=2)
        f.write("\n")


def runtime_info(reports, commit):
    """Return the "Runtime info" block of the release notes, built from srtool's reports."""
    image = f"{SRTOOL_IMAGE}:{srtool_tag()}"
    rows, commands, rustc = [], [], ""
    for path in sorted(reports):
        m = REPORT.fullmatch(os.path.basename(path))
        if not m:
            fail(f"unexpected srtool report name: {path}")
        stem, network = m.groups()
        with open(path) as f:
            data = json.load(f)
        wasm = data["runtimes"]["compressed"]["subwasm"]
        core = wasm["core_version"]
        rustc = data["rustc"]
        rows.append(
            f"| {network} | `{stem}.compact.compressed.wasm` | `{core['specName']}` | "
            f"{core['specVersion']} | {wasm['size']:,} bytes | `{wasm['blake2_256']}` |"
        )
        commands.append(
            f"    # {network}\n"
            f'    docker run --rm --user root -v "$PWD":/build -e PACKAGE={PACKAGE} \\\n'
            f"      -e RUNTIME_DIR={RUNTIME_DIR} -e PROFILE={PROFILE} \\\n"
            f'      -e BUILD_OPTS="--features {network}-runtime" {image} build'
        )
    return "\n".join(
        [
            "## Runtime info",
            "",
            f"- Commit: {commit}",
            f"- srtool image: `{image}`",
            f"- Compiler: {rustc}",
            f"- Profile: `{PROFILE}`",
            "",
            "| Network | File | spec_name | spec_version | Size | blake2-256 |",
            "|---|---|---|---|---|---|",
            *rows,
            "",
            "To reproduce a build, check out the commit and run:",
            "",
            "\n\n".join(commands),
        ]
    )


def notes(version, reports, commit, out):
    """Write the release notes to `out`: the [version] section of CHANGELOG.md, then "Runtime info".

    In Actions they also go to the job summary, so a dry run shows them.
    """
    text, _ = changelog.read_changelog()
    lines = changelog.section(text, version)
    if not lines:
        fail(f"no [{version}] section in {changelog.CHANGELOG}")
    body = "\n".join(lines[1:]).strip() + "\n\n" + runtime_info(reports, commit) + "\n"
    with open(out, "w") as f:
        f.write(body)
    if "GITHUB_STEP_SUMMARY" in os.environ:
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as f:
            f.write(body)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Steps of the release workflow.")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("should-release", help="decide whether this run releases, print what the build needs")
    collect_parser = commands.add_parser("collect", help="check a srtool build and name its files for the release")
    collect_parser.add_argument("runtime", choices=["testnet-runtime", "mainnet-runtime"])
    collect_parser.add_argument("version", help="the version being released")
    collect_parser.add_argument("wasm", help="srtool's compressed wasm")
    collect_parser.add_argument("report", help="srtool's JSON report")
    notes_parser = commands.add_parser("notes", help="write the release notes")
    notes_parser.add_argument("version", help="the version being released")
    notes_parser.add_argument("reports", nargs="+", help="the srtool reports written by `collect`")
    notes_parser.add_argument("--commit", required=True, help="the commit the runtimes were built from")
    notes_parser.add_argument("--out", required=True, help="the file to write the notes to")
    args = parser.parse_args()
    if args.command == "should-release":
        should_release()
    elif args.command == "collect":
        collect(args.runtime, args.version, args.wasm, args.report)
    else:
        notes(args.version, args.reports, args.commit, args.out)
