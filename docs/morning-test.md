# Morning test: what to try on the preview

Open `https://preview.calternal.cloud/ai/?server=1#setup=<setup code>` in a browser on the NetBird network. The code is the one-use setup token of the disposable server; keep it out of chat and screenshots. Each line: do this, expect that. Everything below ran end to end on a fresh server (desktop and phone width); the Threads steps are added separately.

1. **Set up.** The page shows "Set up Calternal" with the code filled in. Type your name, press *Create passkey*, approve the prompt. Expect the app within a few seconds and a "Choose a look" sheet; pick one or *Keep the default*.
2. **Your space.** Expect no space dots (one space, *Personal*) and *Settings > Space: Personal*.
3. **Machines.** *Settings > Machines*: "No machines yet" and *Make a code*; press it for a one-use `calternal machine enroll …` command. Run it on a computer you control to add the machine (it then shows Online with Sandboxed/Direct, Pause, Revoke, and each provider's sign-in state with *Check again* once the server has turn readiness).
4. **Notes.** *Notes > Write the first note*, type under the title and add `#home`. Expect *Not saved yet*, *Saving…*, *Saved* about two seconds after you stop; the tag appears as a chip above the list and filters it. Edit the first heading to rename.
5. **Trash.** The "…" on a note: *Move to Trash* (an Undo appears); trash it again, open *Trash* at the bottom of the list, *Restore*. Type `[[` or `#` in a note to link a note or a tag from the space.
6. **Changes elsewhere, offline.** Edit the same note in a second tab: the first tab updates by itself, or offers *Keep mine* / *Use theirs* if you also typed. Turn the network off while typing: "Can't reach the server. Your text is still here…", then *Saved* when it is back.
7. **People.** *Settings > Invites*: make an invitation (role, how long), copy the link, open it in a private window. Expect "Join Calternal", a name and a passkey, then your guest in the app with their own personal space. Back in *Members and roles* the guest is listed by name: change the role to Editor, *Make owner*, or *Remove*. *Admin > Groups*: make a group, then *Members and roles > Add a group…* gives it a role. *Account*: edit your name.
8. **Look.** *Settings > Appearance*: Dark, Light, another preset; they apply at once and survive a reload. Cmd-K (Ctrl-K elsewhere) opens search; Escape closes it.
9. **Sign out, sign in.** *Account > Sign out* returns to the sign-in screen at the same address; *Sign in with a passkey* brings your note back.
10. **Phone.** Open the link on a phone. A button at the top left opens the list; the app name at the bottom of it switches app (three taps in all).

## Not yet

- Threads steps (the Threads lane adds them); provider sign-in state on Machines needs turn readiness in the server.
- Notes: `@` people in a note. Settings: provider accounts, Agents defaults, plugin install, space instructions.
- Two people typing in one note at once: saves replace the whole note, with the conflict choice above.
- Passkeys on a phone need the preview's HTTPS address; a `localhost` development server only works in the same browser.
