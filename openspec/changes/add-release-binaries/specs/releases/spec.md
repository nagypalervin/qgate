# Spec Delta

## Purpose

Defines how qgate is released: the platforms it ships binaries for,
what pushing a version tag produces, and what every released binary
guarantees, so teams can install qgate without a Rust toolchain.

## ADDED Requirements

### Requirement: Supported platforms

qgate SHALL ship a binary for each of these targets:
- `x86_64-unknown-linux-musl`;
- `aarch64-unknown-linux-musl`;
- `x86_64-apple-darwin`;
- `aarch64-apple-darwin`;
- `x86_64-pc-windows-msvc`;
- `aarch64-pc-windows-msvc`.

CI SHALL run the workspace's test suite on each of these platforms for
every pull request, and a failure on any of them SHALL fail the pull
request's checks.

#### Scenario: A pull request runs the tests everywhere

- **WHEN** a pull request is opened against `main`
- **THEN** CI runs `cargo test --workspace` on Linux x64 and arm64, macOS x64 and arm64, and Windows x64 and arm64
- **AND** a failure on any of them fails the pull request's checks

### Requirement: A version tag makes a draft release

Pushing a tag `v<version>`, where `<version>` equals the workspace
version in `Cargo.toml`, SHALL create a draft GitHub Release named
`qgate <version>` for that tag. It SHALL hold one archive per supported
target and a `SHA256SUMS` file, and nothing SHALL be published until a
maintainer publishes the draft.

#### Scenario: Releasing 0.1.0

- **GIVEN** the workspace version is `0.1.0`
- **WHEN** a maintainer pushes the tag `v0.1.0`
- **THEN** a draft release `qgate 0.1.0` for `v0.1.0` exists, with six archives and `SHA256SUMS`

### Requirement: A tag that does not match the version

A tag `v<version>` whose `<version>` differs from the workspace version
SHALL fail the release workflow before anything is built, with a
message that names both versions, and SHALL create no release.

#### Scenario: A mistyped tag

- **GIVEN** the workspace version is `0.1.0`
- **WHEN** a maintainer pushes the tag `v0.1.1`
- **THEN** the release workflow fails with a message that names `0.1.1` and `0.1.0`
- **AND** no release exists for `v0.1.1`

### Requirement: Archives

Each archive SHALL be named `qgate-<version>-<target>.tar.gz` for Linux
and macOS, and `qgate-<version>-<target>.zip` for Windows. It SHALL
hold one directory, `qgate-<version>-<target>`, with the binary
(`qgate`, or `qgate.exe` on Windows), `README.md`, `LICENSE-MIT` and
`LICENSE-APACHE`.

#### Scenario: The Linux x64 archive

- **WHEN** a user unpacks `qgate-0.1.0-x86_64-unknown-linux-musl.tar.gz`
- **THEN** they get the directory `qgate-0.1.0-x86_64-unknown-linux-musl` with `qgate`, `README.md`, `LICENSE-MIT` and `LICENSE-APACHE`

#### Scenario: The Windows x64 archive

- **WHEN** a user unpacks `qgate-0.1.0-x86_64-pc-windows-msvc.zip`
- **THEN** the directory `qgate-0.1.0-x86_64-pc-windows-msvc` holds `qgate.exe`

### Requirement: Checksums

`SHA256SUMS` SHALL hold one line per archive, in the format that
`sha256sum --check` reads: the lowercase hex SHA-256 of the archive, two
spaces, the archive's file name.

#### Scenario: Checking a download

- **GIVEN** a user downloaded `SHA256SUMS` and one archive into the same directory
- **WHEN** they run `sha256sum --check --ignore-missing SHA256SUMS`
- **THEN** the check reports the archive as `OK`

### Requirement: What a released binary guarantees

Every released binary SHALL be built from the tagged commit with
`--locked`, after the test suite passed on its platform in the same
workflow run. `qgate --version` SHALL print `qgate <version>`. Linux
binaries SHALL be statically linked, so they need no particular C
library.

#### Scenario: The version

- **WHEN** a user runs the binary from any archive of release `0.1.0` with `--version`
- **THEN** it prints `qgate 0.1.0`

#### Scenario: A static Linux binary

- **WHEN** `file` inspects the `qgate` binary from a Linux archive
- **THEN** it reports the binary as statically linked

#### Scenario: Tests failing on one platform

- **GIVEN** a tagged commit whose tests fail on one platform
- **WHEN** the release workflow runs
- **THEN** no draft release is created
