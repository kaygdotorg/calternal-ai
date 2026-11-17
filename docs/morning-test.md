# Morning test: what to try on the preview

Open `https://preview.calternal.cloud/ai/?server=1#setup=<setup code>` in a browser on the NetBird network. The code is the one-use setup token of the disposable server; keep it out of chat and screenshots. Each line: do this, expect that.

1. **Set up.** The page says "Set up Calternal" with the code already filled in. Type your name, press *Create passkey*, approve the prompt. Expect to land in the app within a few seconds, and a "Choose a look" sheet.
2. **Choose a look.** Pick a preset or *Keep the default*. Expect the colours and wallpaper to change at once.
3. **Your space.** Expect the sidebar footer to show no space dots (you have one space, *Personal*), and Settings to say *Space: Personal*.
4. **Machines.** *Settings > Machines*. Expect "No machines yet" and a *Make a code* button. Press it: a one-use command (`calternal machine enroll --server … --code …`) appears. Run it on a computer you control to add the machine; it should then show here as Online with a Sandboxed/Direct choice, Pause and Revoke.
5. **Notes.** Open *Notes*. Expect "A note is a page of Markdown…" and *Write the first note*. Press it, type under the title. Expect the header to go *Not saved yet*, *Saving…*, *Saved* about two seconds after you stop.
6. **Rename.** Edit the first heading. Expect the sidebar title to follow after the save.
7. **Changes from elsewhere.** Open the same note in a second tab and edit it there. Expect the first tab to show the new text by itself, without a reload. If you type in both, expect "Someone changed this note…" with *Keep mine* and *Use theirs*; your typed text stays on screen.
8. **Offline.** Turn the network off while typing. Expect "Can't reach the server. Your text is still here and saves when it is back"; turn it on and expect *Saved*.
9. **Colour scheme and wallpaper.** *Settings > Appearance*: choose Dark, then Light, then another preset. Expect instant changes that survive a reload.
10. **Search and commands.** Press Cmd-K (Ctrl-K elsewhere), type "notes", press Enter. Expect the palette to open, filter, and take you there; Escape closes it and returns focus.
11. **Resize.** Drag the thin bar between the sidebar and the card. Expect the sidebar to follow; arrow keys on the bar move it too.
12. **Account.** *Settings > Account*. Expect your name, your passkey (Remove is disabled while it is the only one), and this browser listed as "this browser". *Add a passkey* adds a second one.
13. **Sign out, sign in.** *Sign out*. Expect the sign-in screen at the same address; *Sign in with a passkey* returns you to your note.
14. **Phone.** Open the same link on a phone (sign in with a passkey on that device, or from a second setup). Expect a button at the top left to open the list; there, tap the app name at the bottom to switch app.

## Not yet

- Threads steps: the Threads lane adds them.
- Notes: delete, restore from trash, tags (`#`), people (`@`) and list rename need server operations that are in progress.
- Settings: profile name, provider accounts, Agents defaults, plugin install, member names and adding people, invites, space instructions, and the whole Admin section are hidden until their operations exist (`docs/ui-shell-status.md` lists each).
- Two people editing one note at once: saves replace the whole note, with the conflict choice above.
- Passkeys on a phone need the preview's HTTPS address; a development server on `localhost` only works in the same browser.
