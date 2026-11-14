# Security Policy

## Reporting a vulnerability

**Please do not open public issues, discussions or pull requests for security problems.**

Report vulnerabilities privately through GitHub: go to the [Security tab](https://github.com/bwalia/fixloop/security) and choose **Report a vulnerability**.

Please include:

- what is affected (version or commit, component, configuration),
- steps to reproduce or a proof of concept,
- the impact you expect.

## What to expect

- We acknowledge reports within **3 working days**.
- We give an initial assessment within **10 working days**.
- We aim to release a fix within **90 days** and coordinate the disclosure date with you.
- We credit reporters in the advisory unless they ask us not to.

Fixes are announced through GitHub Security Advisories and the release notes, with a CVE where applicable.

## Supported versions

Fixloop is pre-1.0. Only the latest release gets security fixes.

## Scope

In scope: the Fixloop code in this repository, its release artifacts and its default configuration. Prompt-injection issues are in scope when they let untrusted content cause an action that policy should prevent (see [docs/security/threat-model.md](docs/security/threat-model.md)).
