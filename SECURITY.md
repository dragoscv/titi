# Security policy

Titi is an end-to-end encrypted walkie-talkie. The relay and the web host never see
voice, text or group keys; security rests on the protocol in `core/` (Noise handshakes,
per-group keys, signed invites — see `docs/adr/0004-security-and-invites.md`), not on
keeping this code secret.

## Reporting a vulnerability

**Do not open a public issue.** Use GitHub's private advisory form:
<https://github.com/dragoscv/titi/security/advisories/new>.

Include a description, the affected component (core, relay, web, Android, desktop, TV)
and a reproduction if you have one. You will get an acknowledgement within 72 hours and a
fix or mitigation plan within 14 days for high/critical issues. Credit is given in the
release notes unless you ask otherwise.

## Scope

In scope: cryptographic protocol flaws, invite/code brute-force, relay abuse (amplification,
room hijack, resume-token theft), web XSS/CSP bypass, Android/desktop IPC and deep-link
handling, supply-chain issues in this repository's build.

Out of scope: denial of service by volume against the public relay, issues requiring a
rooted/jailbroken device, missing hardening headers without a demonstrated impact.

## Supported versions

Only the latest release and `main` receive security fixes.
