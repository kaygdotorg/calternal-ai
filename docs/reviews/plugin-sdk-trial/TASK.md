# Independent author trial 2

Write a Recipe Box plugin using `GUIDE.md`. Produce two author files
named `plugin.ts` and `worker.mjs`. Use the guide's placeholder worker
because this is an offline declaration exercise.

- Use a searchable recipe kind with a title, ingredients as text, and
  Boolean metadata `tried`.
- Show recipes in a bound content list with an empty state.
- Provide an add form with exactly two required fields. Ask for the
  title and ingredients. Send them as operation inputs `title` and
  `ingredients`. The operation creates the recipe kind and declares
  item save as an allowed effect.
- Use a full swipe to mark a recipe tried and the opposite full swipe
  to clear it. Each uses the host metadata patch on the swiped row.
- Connect the app's search control to a search Bind over the recipe kind.

The trial runner follows the install steps in the guide and runs
`sh build-and-check.sh`. The checker reads the emitted package files.
