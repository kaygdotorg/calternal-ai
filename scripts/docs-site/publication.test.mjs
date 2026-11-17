/** Regression checks for the public documentation boundary; run only on a dev VM. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { validate } from './common.mjs';

test('publication refuses infrastructure identifiers and internal evidence', () => {
  for (const text of ['calternal-dev-01', 'kayg@example.org', 'host.vpn.example.org', 'docs/reviews/private.md', 'docs/measurements/private.json', 'docs/open-questions.md']) {
    assert.throws(() => validate(text, 'fixture.md'), /Private material/);
  }
});

test('public example origins and source comments are allowed', () => {
  assert.doesNotThrow(() => validate('<!-- Generated from: contract/operations.json -->\nhttps://example.org/docs/', 'fixture.md'));
});
