# Where the rest of the app most lacks motion (proposal only, 2026-11-19)

Nothing here is built. Ordered by how much a person would notice; each names the order, duration and curve it would use, from the motion tokens, interruptible, with reduced motion at 1 ms.

1. **Opening a thread or a note from a list (phone):** the list slides left and the thread comes in from the right (`--dur-sheet`, `--ease-sheet`), and back the other way; today the card changes at once.
2. **A new message arriving in a thread:** it rises 8 px and fades in (`--dur-base`, `--ease-out`) while earlier messages keep their place; today it appears.
3. **The composer's send:** the text lifts out of the field into the thread (a short translate and fade, `--dur-base`), the send button's arrow fills and returns; the field's height eases back to one line (`--dur-base`).
4. **Rows that leave a list** (archive, move to Trash, snooze): the row collapses its height and fades (`--dur-leave`, `--ease-out`) with the neighbours closing the gap together; Undo reverses it.
5. **Rows that arrive** (a new thread, a note made, an Inbox item): the row grows in (`--dur-base`) and its accent dot pulses once (`--dur-slow`, `--spring`) so the eye finds it.
6. **Switching space:** the sidebar's list cross-fades (`--dur-fast` out, `--dur-base` in) and the account picture swells once, so the change is felt, not just read.
7. **Settings pages:** the heading in the sticky header pill morphs between sections as you scroll (the pill already tracks it; give the label a vertical swap, `--dur-base`), and switching pages in the sidebar slides the page content by the direction of the move.
8. **Toasts and the Undo bar:** enter from the bottom edge with the spring (`--dur-slow`), leave with `--dur-leave`; the Undo countdown is a thin line that drains, not a number.
9. **Menus and popovers from the sidebar footer:** the account picker and the app capsule morph out of the control that opened them (`--ease-morph`, `--dur-morph`) as the composer chips already do.
10. **Empty states and loading:** skeleton rows that breathe (`--dur-pulse`) in place of "Loading…", resolving into real rows with a staggered fade (30 ms apart, first ten rows).
