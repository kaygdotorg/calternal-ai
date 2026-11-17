/**
 * The shell's own state: which regions are open, whether the screen is narrow, and which right-card panel is showing.
 * Desktop choices are remembered; a narrow screen always starts with the sidebar and right card closed so the middle card is what you see.
 */
const KEY = "cal.shell";

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
  /** Opens or closes the sidebar. On a narrow screen opening it closes the right card. */
  toggleSidebar(): void {
    this.sidebarOpen = !this.sidebarOpen;
    if (this.narrow && this.sidebarOpen) this.rightOpen = false;
    this.save();
  }
  /** Opens or closes the right card. On a narrow screen opening it closes the sidebar. */
  toggleRight(): void {
    this.rightOpen = !this.rightOpen;
    if (this.narrow && this.rightOpen) this.sidebarOpen = false;
    this.save();
  }
  /** Shows a right-card panel (opening the card). */
  showPanel(id: string): void {
    this.panel = id;
    this.rightOpen = true;
    if (this.narrow) this.sidebarOpen = false;
    this.save();
  }
  /** Closes whichever region is over the middle card (a narrow screen's back gesture). */
  closeOver(): void {
    this.sidebarOpen = false;
    this.rightOpen = false;
  }
}

/** The shared shell state. */
export const shell = new Shell();
