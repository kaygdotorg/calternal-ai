/** Contacts' collections: the sidebar's groups (each a collection of people), "Everyone" for the library, and for each person the three small collections their page shows (recent mail, chats, events), so "Show all" can open each as a collection of its own. */
import { def, ITEMS, type Item } from "../../data/collections";
import "../mail/data";
import "../calendar/data";
import { CONTACT_ITEMS, PEOPLE } from "./people";

const inGroup = (g: string): Item[] => CONTACT_ITEMS.filter((c) => PEOPLE[c.id]!.groups.includes(g));
def({ id: "ct-fav", name: "Favourites", hue: 330, small: "strip", big: "gallery", pin: true, items: CONTACT_ITEMS.filter((c) => PEOPLE[c.id]!.fav) });
def({ id: "ct-family", name: "Family", hue: 330, library: true, small: "list", big: "gallery", items: inGroup("g-family") });
def({ id: "ct-work", name: "Work", hue: 280, library: true, small: "list", big: "gallery", items: inGroup("g-work") });
def({ id: "ct-neighbours", name: "Neighbours", hue: 150, small: "list", big: "gallery", items: inGroup("g-neighbours") });
def({ id: "ct-services", name: "Services", hue: 28, shut: true, small: "list", big: "list", items: inGroup("g-services") });
def({ id: "ct-birthdays", name: "Birthdays soon", hue: 55, small: "list", big: "list", items: CONTACT_ITEMS.filter((c) => PEOPLE[c.id]!.birthdayOn).sort((a, b) => PEOPLE[a.id]!.birthdayOn!.localeCompare(PEOPLE[b.id]!.birthdayOn!)) });
def({ id: "ct-all", name: "Everyone", hue: 330, library: true, small: "list", big: "gallery", items: CONTACT_ITEMS });
/** A person's recent mail, chats or events, as a collection. */
export const SIDE: { key: "mail" | "chats" | "events"; title: string }[] = [{ key: "mail", title: "Recent mail" }, { key: "chats", title: "Chats" }, { key: "events", title: "Events" }];
for (const c of CONTACT_ITEMS) for (const s of SIDE) {
  const items = PEOPLE[c.id]![s.key].map((id) => ITEMS[id]).filter((i): i is Item => !!i);
  if (items.length) def({ id: `ct-${c.id}-${s.key}`, name: `${s.title}: ${c.title}`, hue: 330, small: "list", big: "list", items });
}
