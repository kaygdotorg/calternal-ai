# Skills and automations

Skills and automations belong to a space. The operations catalog describes the
space identifier, mutation receipt and revision fields, and read results.

A skill is an Agent Skills bundle in `AI/Skills/<name>/`. Its entry point is
`SKILL.md`, with standard YAML frontmatter. The server reads untracked portable
skills from the local tree. The mutation operations manage bundles created
through the content journal. Updates replace the entry point and keep the
other files. Remove places the bundle in that space's Trash. The optional
`metadata.agents` value is a comma-separated string of harness labels.
A skill can describe tools, while the turn's existing authority grants them.

## Confirm a schedule

Preview accepts wording such as “daily at 7:15”, “every weekday at 9”,
“every Monday 8:30”, or “monthly on the 31st at 5pm”. A month without that day
does not run. Bare “monthly at 9” means the first of each month. The Rust
parser consumes the full phrase and returns the wording, readable calendar
expression, creator's account time zone, and next UTC instant. It uses bundled
IANA time-zone data. A fold fires at the earlier instant. A gap is skipped.

Create and schedule edits accept the preview's original words, expression and
zone together. An automation created while the account time zone is unset uses
UTC. After a time-zone change the definition keeps its confirmed zone and
returns the newly selected account zone to the creator. The client can ask
whether to move it and confirm with a new preview. Saving preserves the original
words and canonical expression in `AI/Automations/<name>.md`, alongside
`format: calternal-automation/v1` and the configured permission mode. The body
is the prompt. The content service inserts the stable `calternal-id`.

## Run an automation

A scheduled run uses the original creator's space permissions and entitled
sandbox or explicitly selected enrolled machine. Approve for me is the default.
Approvals use ordinary thread requests. The run's thread carries the automation
UUID and begins with the prompt under the Automation heading. The initial
provider input carries the prompt itself.

A space can hold up to 128 definitions and 32 queued handoffs. An automation
admits one run at a time. The scheduler advances a durable cursor and records
an occurrence in one transaction before handing it to the normal thread and
turn services. Their shared request receipts fence retries. Provider attempts
with an uncertain outcome require ordinary recovery on the same thread.

The scheduler admits a catch-up run only on its scheduled local day. Earlier
misses appear in the most recent status and do not create a backlog of threads.
Pause clears unstarted scheduled work. Enable starts from confirmation time.
Run now uses a separate receipt and leaves the schedule untouched.
If no eligible machine is available, it returns HTTP 409 with reason
`no_machine_available`. Connect a machine and select it for the automation,
then start a new Run now request. Repeating the original request ID returns
the same refusal while its receipt is retained. Loss of
space authority stops scheduling and cancels pending work and running jobs
when checked at dispatch or by the background worker. Re-enable after access
is restored. File changes outside the guarded operation stop execution when
the verified revision no longer matches.
