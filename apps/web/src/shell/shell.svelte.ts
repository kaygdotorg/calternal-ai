/**
 * The shell's own state: which regions are open, whether the screen is narrow, and which right-card panel is showing.
 * Desktop choices are remembered; a narrow screen always starts with the sidebar and right card closed so the middle card is what you see.
 */
const KEY = "cal.shell";
const WKEY = "cal.widths";

/** Which sidebar. */
export type Side = "left" | "right";
/** The width of each sidebar, in unscaled CSS pixels: its default, and the range a person may drag or key it within. */
export const WIDTHS: Record<Side, { def: number; min: number; max: number; prop: string }> = {
  left: { def: 240, min: 200, max: 420, prop: "--side-w" },
  right: { def: 340, min: 260, max: 560, prop: "--right-w" },
};

/** What a narrow screen shows over the middle card: the sidebar (the list), the side panel, or nothing. */
export type Over = "list" | "panel" | null;

class Shell {
  /** True under 760 px wide: sidebar and right card slide over the middle card instead of sitting beside it. */
  narrow = $state(false);
  sidebarOpen = $state(true);
  rightOpen = $state(true);
  /** The search sheet and the keyboard shortcut sheet. */
  searchOpen = $state(false);
  helpOpen = $state(false);
  /** The right card's tab: a panel id from the current app. */
  panel = $state("");
  /** The widths the person chose for this device (absent: the default). */
  widths = $state<Partial<Record<Side, number>>>({});

  /**
   * Sets a sidebar's width (clamped to its range; null puts the default back). `keep` stores it for this device; while a drag is under way it is only applied, once per frame.
   * Applied as the CSS variable the layout already uses, so nothing else knows about it.
   */
  setWidth(side: Side, px: number | null, keep: boolean): void {
    const r = WIDTHS[side];
    if (px === null) delete this.widths[side];
    else this.widths[side] = Math.round(Math.min(r.max, Math.max(r.min, px)));
    const st = document.documentElement.style;
    if (px === null) st.removeProperty(r.prop); else st.setProperty(r.prop, `calc(${this.widths[side]}px * var(--ui-scale, 1))`);
    if (!keep) return;
    try { localStorage.setItem(WKEY, JSON.stringify(this.widths)); } catch { /* private mode: for this page only */ }
  }

  /** Reads the remembered desktop layout and starts following the viewport. Call once. */
  start(): void {
    try {
      const w = JSON.parse(localStorage.getItem(WKEY) ?? "{}") as Partial<Record<Side, number>>;
      for (const side of ["left", "right"] as const) if (typeof w[side] === "number") this.setWidth(side, w[side]!, false);
    } catch { /* none stored */ }
    try {
      const s = JSON.parse(localStorage.getItem(KEY) ?? "{}");
      if (typeof s.sidebarOpen === "boolean") this.sidebarOpen = s.sidebarOpen;
      if (typeof s.rightOpen === "boolean") this.rightOpen = s.rightOpen;
    } catch {
      /* first run */
    }
    // Back and forward move between what is over the middle card: on a narrow screen the sidebar and the side panel are history entries (below).
    addEventListener("popstate", () => {
      if (!this.narrow) return;
      const o = (history.state as { over?: Over } | null)?.over ?? null;
      this.sidebarOpen = o === "list";
      this.rightOpen = o === "panel";
    });
    const mq = matchMedia("(max-width: 759px)");
    const apply = () => {
      this.narrow = mq.matches;
      if (mq.matches) {
        this.sidebarOpen = false;
        this.rightOpen = false;
      }
    };
    apply();
    mq.addEventListener("change", () => {
      apply();
      if (!mq.matches) this.start2();
    });
  }
  private start2(): void {
    this.sidebarOpen = true;
    this.rightOpen = true;
  }

  private save(): void {
    if (this.narrow) return;
    try {
      localStorage.setItem(KEY, JSON.stringify({ sidebarOpen: this.sidebarOpen, rightOpen: this.rightOpen }));
    } catch {
      /* private mode */
    }
  }
  /**
   * Opens or closes the sidebar. On a narrow screen opening it closes the right card, and the sidebar is a history entry: Back (a button, the platform gesture, a swipe) closes it,
   * and Forward opens it again.
   */
  toggleSidebar(): void {
    if (this.narrow) { this.over(this.sidebarOpen ? null : "list"); return; }
    this.sidebarOpen = !this.sidebarOpen;
    this.save();
  }
  /** Opens or closes the right card. On a narrow screen opening it closes the sidebar, and the card is a history entry like the sidebar. */
  toggleRight(): void {
    if (this.narrow) { this.over(this.rightOpen ? null : "panel"); return; }
    this.rightOpen = !this.rightOpen;
    this.save();
  }
  /** Sets a sidebar open or closed on a wide screen (the end of a pull: absolute, where the toggles flip), and remembers it. */
  setOpen(side: Side, open: boolean): void {
    if (side === "left") this.sidebarOpen = open; else this.rightOpen = open;
    this.save();
  }
  /** Shows a right-card panel (opening the card). */
  showPanel(id: string): void {
    this.panel = id;
    if (this.narrow) { this.over("panel"); return; }
    this.rightOpen = true;
    this.save();
  }
  /** What is over the middle card now on a narrow screen. */
  get overKind(): Over { return this.sidebarOpen ? "list" : this.rightOpen ? "panel" : null; }
  /**
   * Puts `next` over the middle card (or nothing) on a narrow screen and keeps history in step: showing something pushes an entry (changing from one to the other replaces it), and
   * hiding it goes Back when its entry is on top, so Forward brings it back. The address does not change. Cost: one history call.
   */
  over(next: Over): void {
    const now = this.overKind;
    if (next === now) return;
    const marked = (history.state as { over?: Over } | null)?.over != null;
    if (next === null && marked) { history.back(); return; } // popstate hides it
    this.sidebarOpen = next === "list";
    this.rightOpen = next === "panel";
    if (next === null) return;
    if (marked) history.replaceState({ over: next }, "");
    else history.pushState({ over: next }, "");
  }
  /** Closes whichever region is over the middle card without touching history: for a navigation that has already moved on (choosing an item in the sidebar). */
  closeOver(): void {
    this.sidebarOpen = false;
    this.rightOpen = false;
  }
  /** Closes whichever region is over the middle card the way Back would. */
  dismiss(): void { this.over(null); }
}

/** The shared shell state. */
export const shell = new Shell();
