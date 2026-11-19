import { getContext, setContext } from "svelte";
import type { ActionUi } from "./actions.svelte";
import type { HostSession } from "../wire/session";

/** What every drawn action needs: the dialogs and invoker of the app, and its open session (the view's own contexts). */
export interface ActionContext { ui: ActionUi; session: HostSession; onshell?: (action: unknown) => void }
export const ACTIONS_KEY = Symbol("description-actions");
export const provideActions = (c: ActionContext) => setContext(ACTIONS_KEY, c);
export const useActions = (): ActionContext => getContext<ActionContext>(ACTIONS_KEY);

/** Reads one host-authorized row image by its ref (`query.row_images`); null when the host has none. Held per app, in memory only. */
export type ImageReader = (ref: string) => Promise<Blob | null>;
export const IMAGES_KEY = Symbol("description-images");

import type { WindowStore } from "../windows/window-store.svelte";
import type { Selection } from "../windows/selection.svelte";
import type { Component } from "svelte";
/** The stores and selections of the open app, for parts that reach them by bind id (child rows, multiple selection) and the host components the page supplies. */
export interface AppContext {
  storeFor(bind: string, parent?: string): WindowStore | undefined;
  selectionFor(bind: string): Selection;
  kindIconFor(bind: string): string | undefined;
  route: Record<string, string>;
  setRoute(name: string, value: string): void;
  components: Record<string, Component<{ config: unknown; id: string }>>;
  /** The shell's own controls: a shell action (route, reload, sign out, sheet) and the app switcher. */
  onshell?: (action: unknown) => void;
  openApp?: (id: string) => void;
}
export const APP_KEY = Symbol("description-app");
export const useApp = (): AppContext => getContext<AppContext>(APP_KEY);
