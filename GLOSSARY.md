# Glossary

Domain terms used in code, docs and reviews. Use these names; don't invent synonyms.

- **Decision table**: a DMN decision expressed as inputs, outputs and rules, with a hit policy. `model::DecisionTable`.
- **Rule**: one row of a decision table: one input entry per input, one output entry per output.
- **Input entry**: a rule's cell for one input. Its text is a unary test, unless its expression language says otherwise.
- **Output entry**: a rule's cell for one output. Its text is an expression that produces the output value.
- **Unary test**: the condition in an input entry, tested against the input's value, e.g. `"EU"`, `<= 5`, `[1..10]`, `not("A","B")` or `-` (any). FEEL syntax by default.
- **Expression language**: the language a cell or input expression is evaluated in (FEEL, JavaScript, JUEL, or an unsupported one), together with where it was set: on the cell, as the file default, or not set (FEEL). The parser resolves it once; nothing else re-derives it. `model::ExpressionLanguage`.
- **Hit policy**: how a table combines matching rules: UNIQUE, FIRST, ANY, PRIORITY, COLLECT (optionally with an aggregation), OUTPUT ORDER, RULE ORDER.
- **Test suite**: the `<name>.tests.json` file next to a DMN file: a decision name plus test cases.
- **Test case**: named inputs and the expected outputs and/or fired rules.
- **Input binding**: how a table input finds its value among named values: by its input expression, falling back to its label.
- **Test session**: the tester's working state for one decision table: the current inputs, the loaded test suite and their evaluated results.
- **Rule coverage**: which rules a test suite's cases fire; a rule fired only by failing cases is counted separately. `suite::Coverage`.
- **Issue**: one finding of the checks, with a severity, a code (e.g. `UNREACHABLE_RULE`) and the decision, rules and column it is about. `lint::Issue`.
- **Fix**: one way to resolve an issue, as edits the user confirms; a fix that deletes a rule or column is destructive and never pre-selected. `fix::Fix`.
- **Fix session**: going through the fixable issues one at a time (apply, skip, stop), re-checking after every fix; each fix is one undo step. Skips last until the file is reloaded.
- **Workspace**: the open file plus everything the editor needs to act on it: the document and its edit history, the active decision, the test session and the fix session. Its views of the document (test results, coverage, fixable issues) refresh themselves; callers never sync them. `workspace::Workspace`.
- **Revision**: a number that changes whenever the document text changes, including undo, redo, discard and reload. A derived view that remembers the revision it was computed at knows it is stale when the numbers differ.
