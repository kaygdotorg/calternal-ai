# Onboarding as a journey: motion and interaction spec (for approval, 2026-11-19)

Owner feedback: "the onboarding could use so much more interaction / animation / motion". This designs onboarding as one sequence that feels alive, not a form in a card. Nothing below the line "Built already" is built until it is approved. Every transition states its order, duration and curve from the motion tokens (`--dur-fast` 150, `--dur-base` 220, `--dur-slow` 300, `--dur-leave` 200, `--dur-morph` 350, `--ease-out`, `--spring`), is a CSS transition or animation of transform and opacity only (no animated blur, no animated backdrop filters), is interruptible, and under reduced motion collapses to the tokens' 1 ms with no loops.

## The idea

The wordmark is the anchor and the stepper is a road. One accent dot (the traveller) is the person; it moves along the road as they go, and everything else responds to it: the wallpaper opens up a little with each step, the card breathes to the size of its content, and the last screen is the arrival.

## Sequence

| Moment | Order and what moves | Duration, curve |
|---|---|---|
| Arrival (first load) | 1 the wordmark writes itself left to right (a clip-path reveal, so it reads as handwriting in the script face), then one soft sheen passes over it; 2 the road draws in from the left, its seven dots rising 45 ms apart; 3 the card rises into place 220 ms after the wordmark starts | wordmark reveal 700 ms `--ease-out` (a single choreographed reveal, longer than `--dur-slow` on purpose), sheen 600 ms; dots `--dur-base`; card `--dur-slow` |
| The traveller | one accent dot glides along the road from step to step (not each dot lighting separately); the road fills behind it; on arriving it releases one ring that grows and fades (a ripple), and the dot it leaves takes its tick (the existing draw-on) | glide `--dur-slow` with `--spring`; fill `--dur-slow` `--ease-out`; ripple `--dur-slow` `--ease-out` (scale 1 to 1.8, opacity .4 to 0) |
| Step to step | the old content leaves toward where the person came from (16 px, fade), the card's height eases to the new content's height (it breathes; no snap), the new content arrives from the side the move goes to (24 px, fade); forward and back mirror each other | leave `--dur-fast`, height `--dur-base`, arrive `--dur-base`, all `--ease-out` |
| The world opens | the wallpaper scales from 1.00 to 1.04 across the steps and drifts a few pixels (a very slow parallax, one transform on the backdrop layer), so progress is felt in the whole screen | change per step `--dur-slow`; the drift is a 40 s alternate loop, paused when the tab is hidden and absent under reduced motion |
| Fields | focus draws an accent underline from the left (scaleX 0 to 1); a valid username shows a tick that draws on at the field's right end; a wrong value nudges the field 3 px once and shows the info panel | underline `--dur-base`; tick `--dur-base`; nudge `--dur-fast` (none under reduced motion; the panel still opens) |
| Primary button | pressed: scale to `--press-scale`; busy: ring grows in before the label (built); on success the label crossfades to a tick for one beat, then the step moves on | press `--dur-fast`, busy `--dur-base`, tick beat 400 ms then the step transition |
| Passkey | while the system sheet is open the key icon beside the button breathes (a soft ring pulse), when it answers the ring completes into the tick, on failure the ring collapses and the info panel opens | pulse 1800 ms (`--dur-pulse`) loop only while the sheet is open; completes in `--dur-base` |
| Recovery codes | the ten codes appear one after another (fade and rise 6 px, 40 ms apart), "Copy" pulses a check where it was | each `--dur-base`; check `--dur-fast` |
| Waiting (machine, provider) | the count-up has a breathing dot; when the machine connects or the provider signs in, the tick draws on inside a soft accent glow that blooms and fades once | breath `--dur-pulse` loop; glow 600 ms `--ease-out`; tick as built |
| Arrival at the end | all dots tick in a quick wave (60 ms apart), the wordmark sheens once, the start button pulses once to invite; on Start the card and wordmark fade and sink (`--dur-leave`) before the app replaces the page, so it is a departure not a cut | wave `--dur-base` each; pulse `--dur-slow` `--spring`; departure `--dur-leave` |

## Interaction (beyond motion)

1. The first field of every screen takes focus when the screen arrives (not on touch, where it would pop the keyboard before the person has read the screen).
2. The eight-digit email code submits itself on the eighth digit; paste of a whole code works.
3. The username is checked as it is typed (the rules are local) with the tick or the sentence under the field.
4. Edge swipe from the left goes back (the same action as the chevron), using the shared press/gesture helpers; not on the first screen.
5. Enter advances; Escape does nothing destructive; the back chevron and Skip are reachable in order.
6. Waiting screens say what they are waiting for and for how long, and move on by themselves the moment the thing is true (as built).

## Cost and checks

One infinite animation at a time at most (the breathing dot or the parallax drift, never both on screen), transform and opacity only, none with reduced motion, battery saver (`prefers-reduced-motion`) or a hidden tab. Onboarding is not in the quiet list, but the same discipline applies: no task over 50 ms while it moves, checked on the phone Simulator and with the profiler. Frames on issue #15.

## Built already (before this spec was asked for; kept, small)

Back chevron, the info panel (`Callout`), passkey wording, step-to-step slide with direction, arrival order (wordmark, dots, card), the line between dots filling, the busy ring on buttons, the screen-to-screen slide inside "Who you are". The rest of the table is waiting for approval.
