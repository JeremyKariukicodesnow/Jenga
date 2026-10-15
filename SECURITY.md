# Security policy

0.8.0 is an early-access release without a formal security audit or guaranteed response SLA. Fixes target the current development version; there is no long-term support promise yet.

**Do not publish exploit details, tokens or private designs in an issue.** Use this repository's Security → Report a vulnerability private reporting feature. If unavailable, contact the maintainer through their GitHub profile and request a private channel before sharing sensitive information. Do not guess an email address.

Include the version, OS, minimal steps, impact and a sanitized sample. Do not attack third-party systems, access other users' data or distribute weaponized samples. Give the maintainer an opportunity to reproduce and coordinate a fix; no bounty or legal safe-harbor promise is made here.

Open only trusted projects/assets. Imported files and external tool output are untrusted input. Jenga's local authenticated automation endpoint is not intended for internet exposure. Never put AI-provider keys, signing keys or repository tokens in projects or issue attachments.

Maintainers should triage privately, reproduce in isolation, fix with a regression test, prepare a security advisory and patched release, and credit the reporter with consent. Review dependencies and third-party source availability before distribution. Tests and dependency alerts do not establish that a release is secure.
