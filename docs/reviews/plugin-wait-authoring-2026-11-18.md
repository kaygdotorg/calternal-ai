# Round three wait authoring follow-up

The trial author used object-style Form syntax. Both local commands passed,
but the job failed with only a generic GenUI message. The live regression
reproduced that failure before the fix.

- Jobs now return the bounded compiler or evaluator diagnostics in the existing
  failure result. Each row includes the form path, source position, expected
  syntax and the token found. The job store schema is unchanged.
- `plugin check` validates the starter's static `question.form.json`.
  `plugin test` validates dynamic question forms returned during SDK fixture
  invocations with the same host checker.
- `plugin new NAME --template question` supplies the manifest, typed handler,
  answer form, fixture and README. `jobs` and `waits` share that example
  with the scaffold. The SDK helper types admission and the run schema
  narrows the answer after checking the signal kind.

Dev-01 passed 9 focused job cases, 17 SDK tests, the guide test, and the
standalone question template fixture. Both Rust formatting checks, the
generated contract and all 420 generated documentation pages passed.

## Simplification opportunities

Keep static questions as files the checker and worker share. Reuse the
same example source for the scaffold and every public guide topic.
