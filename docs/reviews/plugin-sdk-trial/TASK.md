# Independent author trial

Build a Habit Tracker plugin using the installed Calternal Plugin SDK and
the guide. Start with a new `plugin.ts`. Your output is the emitted
`dist/` directory.

- Declare a searchable habit kind. Its metadata includes a title, a
  cadence as text, and a Boolean `done_today` field.
- Show the habits in a bound content list with an empty state.
- Provide an add form with exactly two required fields. Ask for the
  title and cadence. Submit both values to an operation that creates the
  habit kind and declares item save as an allowed effect. Name the
  operation inputs `title` and `cadence`.
- Provide a full swipe to mark a habit done today, and a full swipe on the
  opposite side to clear it. Use the host metadata patch on the swiped row.
- Connect the app's search control to a search Bind over the same kind.

This exercise checks descriptions. Any required worker can be a small
placeholder that exits with failure. Date rollover and actual item creation
belong to the runtime. See `GUIDE.md` to install, build, and check.
