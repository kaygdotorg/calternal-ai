/**
 * Adapt an authenticated, validated host invoke to the handler context.
 * The adapter supplies a broker call already bound to that invocation.
 */
export function authorContext(message, call) {
  const { author_context: author, job_context: job, alarm } = message;
  return Object.freeze({
    author: Object.freeze({ ...author }),
    ...(job && { job: Object.freeze({ ...job }) }),
    ...(alarm && { alarm: Object.freeze({ ...alarm }) }),
    call,
  });
}
