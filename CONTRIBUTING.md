# Contributing

Thank you for helping improve Jenga. Jeremy (@JeremyKariukicodesnow) maintains upstream and decides which changes are merged and released.

1. Check existing issues. Discuss substantial changes before implementation.
2. Fork the repository; branch from main. Keep changes focused and explain the user-visible problem.
3. Preserve native unknown fields, UUID identity, wiring/net membership and human edits. Never hide errors by dropping objects or replaying uncertain mutations.
4. Add meaningful regression fixtures/tests. Use original or redistributable minimal fixtures; remove credentials, personal information and customer designs.
5. Run `cargo fmt --all --check` and `cargo test --workspace --locked`. Run `cargo clippy --workspace --all-targets --locked -- -D warnings`, `node --test scripts/test-release-links.mjs` and `./scripts/test-download-aliases.ps1` (PowerShell). Windows checks and strict Clippy are required; do not suppress failures to merge.
6. Include testing evidence, compatibility risks and documentation changes in the pull request. Disclose AI assistance and personally review/test its output.

New dependencies/assets must include origin, exact version/commit, license, attribution and distribution obligations. Downloaded code and generated parts are not automatically trustworthy. Do not add compiled binaries, private projects or large libraries to Git. Do not change the version or publish a release in an ordinary feature PR.

By submitting a contribution, you agree to license it under the applicable repository license and confirm that you have the right to contribute it. This is not a copyright assignment or a CLA. Third-party license notices remain intact.

Passing CI does not guarantee acceptance or correctness. A maintainer reviews and merges PRs. Contributors do not receive write access automatically. Follow CODE_OF_CONDUCT.md and use SECURITY.md for private vulnerability reports.
