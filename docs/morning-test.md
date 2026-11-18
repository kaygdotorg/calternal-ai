# Morning test: what to try on the preview

Open **`https://preview.calternal.cloud/ai/#setup=<setup code>`** (replace `<setup code>` with the one-use code of the first-owner setup; the preview build talks to its real server by default, so no flag is needed; the design fixtures you have been reviewing all night, with the mock threads and notes, are now at **`https://preview.calternal.cloud/ai/?server=0`**: without that flag the page goes to the real server and starts at "Set up Calternal") in a browser on the NetBird network. The code is the one-use setup token of the disposable server; keep it out of chat and screenshots. Each line: do this, expect that. Everything below ran end to end on a fresh server at desktop and phone width; the Threads steps are added separately.

## What changed since last night

The things you reported, checked in a headed GPU Chrome at 1500x970, dark, default wallpaper, before (last night's start) against now; screenshots for each are on [issue #15](https://git.kayg.org/kayg/calternal-ai/issues/15).

| You reported | Verdict |
| --- | --- |
| A line, strip or band around either sidebar | Fixed (a teal strip at the left edge and an orange one at the right are gone) |
| Top fade | Fixed: text fades out gradually under the bar |
| Sidebar buttons and pills | Not reproducible: round glass buttons in both builds, no defect seen |
| Resize handles | Fixed: a slim handle shows in the gap on hover |
| Hover actions on a thread row | Still there: the link icon overlaps the end of a long title (Threads lane has it) |
| Focus rings from a mouse click | Fixed: none after clicking a pill, a tab or a chip |
| Pill bounce | Not reproducible (no rebound in the press frames) |
| Popover morph | Fixed: the popover grows out of its chip |
| Found on the way | The active-children pill floats over the last message lines (Threads lane); with 50 spaces the footer dots overflow (your call, see the issue) |

1. **Set up.** The page shows "Set up Calternal" with the code filled in. Type your name, press *Create passkey*, approve the prompt. Expect the app within a few seconds and a "Choose a look" sheet; pick one or *Keep the default*.
2. **Your space.** Expect no space dots (one space, *Personal*) and *Settings > Space: Personal*.
3. **Machines.** *Settings > Machines > Make a code* gives a one-use `calternal machine enroll …` command; run it on a computer you control. The machine then shows Online with Sandboxed/Direct, Pause, Revoke, and each provider's sign-in state with *Check again*.
4. **Notes.** *Notes > Write the first note*, type under the title and add `#home`. Expect *Not saved yet*, *Saving…*, *Saved* about two seconds after you stop; the tag appears as a chip above the list and filters it. Edit the first heading to rename.
5. **Trash.** The "…" on a note: *Move to Trash* (an Undo appears); trash it again, open *Trash* at the bottom of the list, *Restore*. Type `[[` or `#` in a note to link a note or a tag from the space.
6. **Elsewhere, offline.** Edit the same note in a second tab: the first updates by itself, or offers *Keep mine* / *Use theirs* if you also typed. Network off while typing: "Can't reach the server. Your text is still here…", then *Saved* when it is back.
7. **People.** *Settings > Invites*: make an invitation, copy the link, open it in a private window: "Join Calternal", a name and a passkey, then your guest in the app with their own personal space. In *Members and roles* the guest is listed by name: change the role, *Make owner*, *Remove*. *Admin > Groups* makes a group that *Add a group…* gives a role. *Account* edits your name.
8. **Look.** *Settings > Appearance*: Dark, Light, another preset apply at once and survive a reload. Cmd-K (Ctrl-K elsewhere) searches notes, threads and spaces.
9. **Sign out, sign in.** *Account > Sign out* returns to the sign-in screen at the same address; *Sign in with a passkey* brings your note back.
10. **Phone.** Open the link on a phone. A button at the top left opens the list; the app name at the bottom of it switches app (three taps in all).

## Not yet

- Threads steps (the Threads lane adds them). The Notes list shows the newest 100 notes (find, tags and `[[` reach the rest) until the server pages it.
- `@` people in a note; provider accounts, Agents defaults, plugin install, space instructions in Settings.
- Two people typing in one note at once: saves replace the whole note, with the conflict choice above.
- Passkeys on a phone need the preview's HTTPS address; a `localhost` server only works in the same browser.
