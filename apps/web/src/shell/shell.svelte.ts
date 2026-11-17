/**
 * The shell's own state: which regions are open, whether the screen is narrow, and which right-card panel is showing.
 * Desktop choices are remembered; a narrow screen always starts with the sidebar and right card closed so the middle card is what you see.
 */
const KEY = "cal.shell";

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

  /** Reads the remembered desktop layout and starts following the viewport. Call once. */
  start(): void {
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
