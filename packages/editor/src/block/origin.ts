import { Annotation } from "@codemirror/state";
import type { Origin } from "../origins";
/** The origin of a transaction (user, remote, replay, command); unannotated transactions are the person's own typing. */
export const origin = Annotation.define<Origin>();
