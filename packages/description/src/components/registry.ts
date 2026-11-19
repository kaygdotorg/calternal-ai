import type { Component } from "svelte";
import TurnTranscript from "./TurnTranscript.svelte";
import Composer from "./Composer.svelte";
import RequestCard from "./RequestCard.svelte";

/** The host components this package draws itself. The item editor and the sealed HTML frame are the page's (a real editor, a sandboxed frame): they stay labelled slots until the page supplies them. */
export const hostComponents: Record<string, Component<{ config: never; id: string }>> = { "turn-transcript": TurnTranscript as never, composer: Composer as never, "request-card": RequestCard as never };
