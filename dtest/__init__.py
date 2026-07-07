"""dtest — shared test-support package for this monorepo's lager tests.

Mirrors the firmware-mono convention of a repo-level helper package that
directory-module tests import (`from dtest... import ...`). Staged next to
the test module by the Stout PR Reviewer's include mechanism (the reviewer's
`testIncludes` config, the analog of a `.lager` includes entry).
"""
