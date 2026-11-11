# Contributing

In a regular PR:

- add entries for your changes to [CHANGELOG.md](CHANGELOG.md) under `[Unreleased]`, or tick
  "Does not require a CHANGELOG entry" in the PR description;
- add storage migrations to `migrations::Unreleased`;
- don't change `spec_version`.

A release PR later moves these entries and migrations into the next version and bumps
`spec_version`. [RELEASE.md](RELEASE.md) describes it.
