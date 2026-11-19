/**
 * The account menu's entries and what choosing one does: who is signed in (it opens Settings, Account), the spaces (the current one checked), Settings, and Sign out. One definition for the two places that show it:
 * the account picture's dropdown in the sidebar and the native shell's account menu (shell/chrome). Reads `self` and `spaces`; call it inside an effect or a `$derived` so it follows them.
 */
import type { MenuItem } from "@calternal/ui";
import { navigate } from "../lib/route.svelte";
import { self, signOutNow } from "./self.svelte";
import { spaces } from "./spaces.svelte";

/** The entries, top to bottom: the signed-in person, each space, Settings, Sign out (danger). */
export function accountItems(): MenuItem[] {
  return [
    { id: "account", label: self.name, sub: self.email ?? `@${self.username}`, icon: "user" as const },
    ...spaces.list.map((s) => ({ id: `space:${s.id}`, label: s.name, checked: s.id === spaces.current.id })),
    { id: "settings", label: "Settings", icon: "settings" as const },
    { id: "signout", label: "Sign out", icon: "signout" as const, danger: true },
  ];
}

/** Does what choosing the entry `id` means; returns false for an id that is not one of ours. */
export function pickAccountItem(id: string): boolean {
  if (id === "settings") navigate("/settings/appearance");
  else if (id === "account") navigate("/settings/account");
  else if (id === "signout") signOutNow();
  else if (id.startsWith("space:")) { spaces.switchTo(id.slice(6)); navigate("/threads"); }
  else return false;
  return true;
}
