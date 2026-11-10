# Multi-row expectations are JSON arrays per output

Status: accepted, 2026-11-10

## Context

Tables with the hit policies COLLECT (with no aggregation), RULE ORDER and OUTPUT ORDER return one result row per fired rule. In a test case, `expected` maps each output name to a single JSON value, so only the first row was saved and compared. Changes in the other rows were caught only through `expectRules`.

## Decision

For those hit policies, each expected output is a JSON array with one entry per result row, e.g. `"Fee": [4.5, 2]`. A scalar is still accepted and means exactly one row, so existing suites keep working. New saves and Accept write arrays for those tables. COLLECT with an aggregation keeps a single scalar, compared against the aggregate.

There is no ambiguity, because output values are never lists.

## Alternatives

- `expected` as a list of row objects for multi-row tables: it changes the shape of the file per hit policy and breaks existing suites.
- Keep comparing row 0 only and rely on `expectRules`: rows after the first stay unchecked.
