/** Navigation the Mail screens ask for, shared by the sidebar and the middle card so both do the same thing. */
import { navigate } from "../../lib/route.svelte";
import { shell } from "../../shell/shell.svelte";
import type { MailEffects } from "./store.svelte";

/** Opening a message or the account form is a page of the app (`/mail/<uid>`, `/mail/setup`); on a phone the list steps aside to show it, as Notes does. */
export const effects: MailEffects = {
  open: (uid) => { navigate(`/mail/${uid}`); if (shell.narrow && shell.sidebarOpen) shell.closeOver(); },
  setup: () => { navigate("/mail/setup"); if (shell.narrow && shell.sidebarOpen) shell.closeOver(); },
  leave: () => navigate("/mail"),
};
