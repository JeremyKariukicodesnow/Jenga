# Platforms and builds

Jenga 0.8.2 provides a complete **Windows x64 EXE installer** and portable ZIP. They include symbols, footprints, 3D libraries and the native PCB backend; a separate KiCad installation is unnecessary. Earlier Windows installers remain available on their individual release pages.

| Platform | Availability |
| --- | --- |
| Windows x64 | Full early-access installer and portable ZIP |
| Windows ARM64 | Experimental native compilation and CLI launch check |
| Linux x64 and ARM64 | Experimental native compilation and CLI launch check |
| macOS Intel and Apple Silicon | Experimental native compilation and CLI launch check |

The **Experimental native platform builds** GitHub Actions workflow builds the desktop, CLI and connection bridge on native runners. Successful runs provide temporary binary artifacts to signed-in GitHub users. A green compile/CLI smoke check is not desktop, clipboard, installer or PCB-backend validation. These artifacts are **not complete offline packages**, signed macOS applications, DMGs or AppImages. Do not substitute them for the Windows installer or assume all desktop features work.

For experimental source builds, install stable Rust (at least the workspace `rust-version`) and native development tools, then run `cargo build --release --locked`. Linux needs pkg-config, X11, Wayland, xkbcommon, OpenGL and D-Bus development packages; macOS needs Xcode command-line tools; Windows ARM64 needs the ARM64 MSVC toolchain and SDK. Build artifacts are in `target/release`.

Experimental packages need separately provisioned symbol/footprint/model libraries. Native PCB DRC, conversion and complete 3D assemblies require an appropriate KiCad 10 CLI/backend for that operating system; set `JENGA_KICAD_CLI` explicitly when it is not on PATH. The bundled Windows x64 backend must not be copied to Linux or macOS. Native runtime distribution, licensing, packaging, signing and desktop QA remain prerequisites for full platform releases.

Source and exact commit history: https://github.com/JeremyKariukicodesnow/Jenga

Builds and their status: https://github.com/JeremyKariukicodesnow/Jenga/actions/workflows/platforms.yml
