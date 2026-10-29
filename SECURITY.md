# Security

## Reporting a vulnerability

Please report security problems privately through GitHub's
[private vulnerability reporting](https://github.com/udvlp/UmlCraft/security/advisories/new),
not in a public issue. Include the UmlCraft version, the platform and, if possible, a file or
steps that show the problem.

You get an answer within two weeks. Once a fix is released, the advisory is published with it.

## Supported versions

Only the latest release gets fixes.

## Scope

UmlCraft opens files from others: its own `.ucraft` projects, Astah's `.asta` files and source
code for the import. A file that crashes UmlCraft, makes it hang or use unbounded memory, or makes
it write outside the chosen target is in scope. The Astah export runs Astah's own Java runtime;
problems in Astah itself belong to its vendor.
