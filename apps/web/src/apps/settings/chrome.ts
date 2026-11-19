/**
 * Settings' side of the native chrome contract (shell/chrome): the pages the person can see (the User, Admin and space groups of the Settings sidebar, one selected) as rows, the open page's name as the header title, and the
 * search text narrowing the pages by name. Choosing a row opens that page. The pages themselves are the page's own content in the card (the native chrome only replaces the sidebar and the header). Pure of the DOM.
 */
import { navigate, segments } from "../../lib/route.svelte";
import type { ChromeBody, ChromeEvent, ChromeProvider } from "../../shell/chrome/types";
import { visibleSections } from "./registry";

/** The Settings app's provider for the native chrome. */
export const settingsChrome: ChromeProvider = {
  describe(query: string): ChromeBody {
    const q = query.trim().toLowerCase();
    const current = segments()[0] === "settings" ? segments()[1] : undefined;
    const groups = visibleSections();
    const page = groups.flatMap((g) => g.pages).find((p) => p.id === current);
    return {
      header: { title: page?.name ?? "Settings" }, selectors: [],
      sections: groups.map((g) => ({ id: g.id, title: g.name, rowKind: "navigation" as const, rows: g.pages.filter((p) => !q || p.name.toLowerCase().includes(q)).map((p) => ({ id: p.id, title: p.name, icon: p.icon, selected: p.id === current })) })).filter((s) => s.rows.length),
      search: { placeholder: "Search settings" },
    };
  },
  handle(ev: ChromeEvent): boolean {
    if (ev.type !== "row" || !ev.id) return false;
    navigate(`/settings/${ev.id}`);
    return true;
  },
};
