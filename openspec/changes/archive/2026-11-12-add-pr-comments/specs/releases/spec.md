# Spec Delta

## MODIFIED Requirements

### Requirement: Archives

Each archive SHALL be named `qgate-<version>-<target>.tar.gz` for Linux
and macOS, and `qgate-<version>-<target>.zip` for Windows. It SHALL
hold one directory, `qgate-<version>-<target>`, with the binary
(`qgate`, or `qgate.exe` on Windows), `qgate-pr-comment.ps1`,
`README.md`, `LICENSE-MIT` and `LICENSE-APACHE`.

#### Scenario: The Linux x64 archive

- **WHEN** a user unpacks `qgate-0.2.0-x86_64-unknown-linux-musl.tar.gz`
- **THEN** they get the directory `qgate-0.2.0-x86_64-unknown-linux-musl` with `qgate`, `qgate-pr-comment.ps1`, `README.md`, `LICENSE-MIT` and `LICENSE-APACHE`

#### Scenario: The Windows x64 archive

- **WHEN** a user unpacks `qgate-0.2.0-x86_64-pc-windows-msvc.zip`
- **THEN** the directory `qgate-0.2.0-x86_64-pc-windows-msvc` holds `qgate.exe` and `qgate-pr-comment.ps1`
