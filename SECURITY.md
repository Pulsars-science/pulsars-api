# Security policy

## Reporting a vulnerability

**Do not open a public issue, discussion or pull request for a security
problem.**

Report it privately through GitHub:
[Report a vulnerability](https://github.com/Pulsars-science/pulsars-api/security/advisories/new)
(repository → Security → Advisories → "Report a vulnerability").

Please include:

- what is affected (endpoint, component, version or commit);
- how to reproduce it, step by step;
- the impact you expect (data exposed, privileges gained…);
- whether you have shared it with anyone else.

## What happens next

- We acknowledge the report within **5 working days**.
- We confirm or dismiss it, and keep you informed while we fix it.
- We publish a security advisory once a fix is deployed, crediting you unless
  you prefer otherwise.
- Please give us reasonable time (90 days by default) to fix the issue before
  disclosing it.

## Scope

In scope: this repository and the service it runs at `api.pulsars.science`.
Of particular interest:

- any way to obtain exercise solutions, expected answers or exam corrections
  through the API;
- any way to read personal data beyond public profiles;
- authentication bypass (token verification, key handling);
- game results that a client can forge.

Out of scope: denial of service by volume, findings from automated scanners
without a demonstrated impact, and the Laravel site `pulsars.science` (report
those to its own repository).

Please test only against your own local instance, never against production
accounts other than your own.

## Supported versions

Only the latest release and the `main` branch receive security fixes.
