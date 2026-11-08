# WFS Notch · REA + Craft Anything + LeanREA research workflow

**Project:** WFS Notch (original Rust + Tauri 2 implementation).  
**Reference:** user-supplied Poni 1.5.0 portable/setup Windows executables, for permitted interoperability research.  
**State:** Toolchain documentation and install/audit scripts only. Installing them is not proof of completed reconstruction.

## Verified upstream tool sources

| Tool | Official repository | Primary contribution |
| --- | --- | --- |
| REA (Reverse Engineer Anything) | https://github.com/morluto/rea | MCP/CLI evidence, Rust binary provenance, PE imports, decompiler bridge (Ghidra/Hopper/IDA) |
| Craft Anything (ArtCraft) | https://github.com/storytold/craft-anything | Study → Build → Check → Refine; comparison of observed UI and implementation; early alpha |
| LeanREA | https://github.com/aicodeking/leanrea | Small skill with direct command routing (minimal MCP/context overhead) |
| Ghidra | https://github.com/NationalSecurityAgency/ghidra | Native disassembly and pseudocode, local use |
| WFS Notch | https://github.com/wawanwfs/WFS-Notch | Independent Rust/Tauri implementation, not a repackaged Poni executable |

**Important distinctions:** LeanREA is an alternative lightweight research skill—not a renamed REA binary. REA itself is a Node.js package, not a Rust compiler or full GUI clone generator. Craft Anything is itself written in Rust and may invoke a coding agent, but its desktop behavior checks remain partly unverified upstream. Avoid downloading similarly named unofficial REA forks.

## Windows 10/11 setup (no winget needed)

1. Install official Node.js with an REA-supported version: **22.23.4+** (22.x), **24.22.1+** (24.x), or **26.12.1+** (26.x); ensure `node` and `npm` work in a new PowerShell window.
2. In PowerShell, from the cloned WFS Notch root, run:

   ```powershell
   Set-ExecutionPolicy -Scope Process Bypass
   .\03_TOOLS\Install-REA-Craft-Windows.ps1 -InstallREA -InstallCraftAnything
   ```

   The script downloads **Craft Anything v0.5.0 Windows x64 portable** from the official release and validates its SHA-256 against the official manifest before extraction. It installs the official `rea-agents` npm package globally only if `-InstallREA` was requested. npm dependencies may execute install scripts; review the official project and use a VM if you cannot trust them.

3. Install Ghidra and its compatible 64-bit JDK manually (not bundled with WFS Notch), then set environment variables to your actual folders:

   ```powershell
   $env:GHIDRA_INSTALL_DIR = "C:\tools\ghidra_12.1.4_PUBLIC"   # example only
   $env:JAVA_HOME = "C:\Program Files\Java\jdk-21"           # example only
   rea doctor --provider ghidra --json
   rea providers --json
   ```

4. Review before adding REA MCP/skill to your coding agent:

   ```powershell
   rea setup --client codex --dry-run
   rea setup
   ```

   The interactive setup changes coding-agent configuration; do not store tokens or agent-specific configuration in the public repo. REA does not install Ghidra for you.

5. From your local Windows workstation, analyze the reference **without executing it**:

   ```powershell
   .\03_TOOLS\Audit-Reference-With-REA.ps1 -ReferenceExe "$env:USERPROFILE\Downloads\Poni_1.5.0_portable.exe"
   .\03_TOOLS\Audit-Reference-With-REA.ps1 -ReferenceExe "$env:USERPROFILE\Downloads\Poni_1.5.0_portable.exe" -Deep
   ```

   The `-Deep` option requires a functioning Ghidra provider; large Rust/Tauri binaries can take substantial resources to analyze. Both commands generate evidence at `%LOCALAPPDATA%\WFSNotch\private-rea-audit\`. The script never launches the target executable.

6. Use the unpacked Craft Anything application to compare *observed* Poni UI screens and user actions with WFS Notch. Launch the reference only on a separate, permissioned, disposable Windows test VM if you decide to perform runtime observation. Screenshots, animations and test timings need to come from real observations, not guesses. Craft Anything can use its own coding-agent configuration; for source changes, continue using Rust/Tauri and normal GitHub PR/CI gates.

## Investigation and feature-parity methodology

For each module in [feature matrix](REA_FULL_PARITY_MATRIX_V150.md):

1. **Observe:** screenshot reference at multiple screen sizes and display scaling factors (100%, 125%, 150%), with compact and expanded states, hover, drag, focus, foreground and fullscreen behavior. Record OS build, monitor topology, DPI, action timing, and permissions.
2. **Inspect:** use REA's static PE/Rust/Ghidra tools to correlate likely function boundaries, Tauri commands, resources, runtime dependencies and Windows API calls. Use LeanREA for quick inspections when MCP startup overhead is not worthwhile. Mark every finding as **Observed**, **Inferred**, or **Unknown**.
3. **Specify:** derive observable acceptance tests, state transitions, permissions and error handling. Explicitly separate reference-proven behaviors from independent design decisions.
4. **Implement:** write original Rust/Tauri code, unique WFS Notch identity, accessible components, safe native APIs, and reversible local state migrations.
5. **Compare:** collect screenshots and native Windows run logs; calculate alignment and color differences *and* manually test animations, input response, OS interactions, sleep/resume, multi-monitor and media session edge cases. A visual screenshot similarity score alone cannot establish functional parity.
6. **Validate:** run browser tests, Rust unit tests, Clippy, rustfmt, GitHub Windows x64 builds, then user-mode smoke tests on actual Windows machines or isolated VMs. If a test is only structural, label it accordingly.
7. **Report:** update a gap matrix with a verifiable reference, implementation commit, test result and known limitations. Never report 100% until all reference features pass verified behavioral and visual parity gates.

## Privacy and release controls

- Keep supplied Poni EXEs, proprietary resources, extracted code, pseudocode and raw screenshots **outside** the public WFS Notch repository. Do not commit them or upload to GitHub Actions.
- Do not bypass DRM, app licensing or user logins. Use tools only where examination is permitted; consult the applicable software license and law.
- No public Cloudflare Tunnel or remote shell on GitHub-hosted Actions. Artifacts/logs are the preferred CI evidence.
- Do not run downloaded binaries automatically as part of the setup script. The portable Craft Anything archive is extracted but not executed.
- Keep WFS Notch logos and brand original; do not copy Poni's proprietary visual assets.
- GitHub CI does not run REA against Poni because the user-supplied reference executable is not part of the repository and must remain local.

## Practical limitations today

- The reference EXEs were previously statically examined, but neither reference nor Craft Anything/REA/Ghidra was executed in the user's real Windows session here. A tool installation script is a *reproducible setup path*, not an installation claim.
- The automated Windows build certifies compile and packaging only. It does not certify actual music session detection, tray behavior, UI pixel parity, Windows notification interception or multi-monitor handling.
- Craft Anything is upstream early alpha and cannot guarantee full app behavior comparison.
- The pending native media sessions PR is separate from this research toolchain; do not merge it without fixing its failed CI checks.

**Source instructions:** [REA CLI guide](https://github.com/morluto/rea/blob/main/docs/getting-started/cli.md), [REA install guide](https://github.com/morluto/rea/blob/main/docs/getting-started/installation.md), [Craft Anything guide](https://github.com/storytold/craft-anything#how-it-works).
