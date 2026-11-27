/**
 * Small state the Photos pages share: which list a viewer was opened from (so its strip and its arrows walk that list, and Back returns to it), and the favourites a person toggled in this session. A window is the key; nothing is stored.
 */
import { META } from "./data";

class PhotoState {
  /** The window's last list: a library list id or an album id ("" is all photos). */
  from = $state<Record<string, string>>({});
  /** Favourite toggles made in this session, over the fixture's own. */
  fav = $state<Record<string, boolean>>({});
  isFav(id: string): boolean { return this.fav[id] ?? META[id]?.fav ?? false; }
  toggle(id: string): void { this.fav[id] = !this.isFav(id); }
}
export const ps = new PhotoState();
