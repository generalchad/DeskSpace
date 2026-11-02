# Security policy

## Supported versions

DeskSpace is pre-1.0. Only the latest release receives security fixes; fixes
ship as a new patch version, and the in-app updater offers it.

| Version | Supported |
|---------|-----------|
| Latest release | Yes |
| Anything older | No |

## Reporting a vulnerability

Please do **not** open a public issue, discussion, or pull request for a
security problem.

Report it privately through GitHub instead:
[**Report a vulnerability**](https://github.com/generalchad/DeskSpace/security/advisories/new)
(the Security tab of the repository). Only the maintainers can see the report,
and you need nothing beyond a GitHub account; no email address is exchanged.

Include what you can of:

- the DeskSpace version (Settings > About, or the release you installed) and
  the Windows version;
- what an attacker can do, and what they need first (local access, a crafted
  file, a network position, ...);
- steps to reproduce, or a proof of concept;
- any fix you have in mind.

## What to expect

- An acknowledgement within 7 days.
- An assessment, and a plan or a reason for declining, within 30 days.
- A fix in a patch release, followed by a published GitHub security advisory
  that credits you unless you ask not to be named.

Please give us a reasonable chance to release a fix before disclosing the
problem publicly; we aim for no more than 90 days from the report.

## Scope

Of particular interest:

- the update path: the update check, the `latest.json` feed, signature
  verification of the downloaded installer, and the installer itself
  (`crates/deskspace-update`, `RELEASING.md`);
- the icon set download and its signature check (`deskspace_update::fetch_signed`);
- file handling: moving files into and out of the private store, paste, drag
  and drop, delete, and anything that could touch a file outside the user's
  intent;
- the desktop takeover and the `deskspace-watchdog` helper;
- parsing the configuration file and shell data (shortcuts, drag formats).

Out of scope: problems that need an attacker who already runs code as the same
user, and bugs in Windows or third-party dependencies (report those upstream;
`cargo deny` tracks known advisories in our dependency tree).

## Privacy

DeskSpace collects no telemetry. Its only network access is the update check,
which asks GitHub for the latest release, and the one-time download of a space
icon set, which asks first (Settings > General, "Icon downloads") and is
verified against the same signing key. Neither sends anything about you or your
files. Anything that contradicts this is a security bug; please report it as
above.
