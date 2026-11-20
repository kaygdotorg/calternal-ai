// Bench-only trivial plugin: the smallest useful handler, no imports.
export const handlers = {
  'trivial.echo': async input => ({ echo: input }),
};
