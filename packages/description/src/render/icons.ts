import { ICONS, type IconName } from "@calternal/ui";

/** The design system's icon for a catalogue name, or undefined when the set has none (the row then keeps its icon column with a neutral placeholder rather than dropping the icon). */
export function knownIcon(name: string | undefined): IconName | undefined {
  return name !== undefined && Object.prototype.hasOwnProperty.call(ICONS, name) ? (name as IconName) : undefined;
}
