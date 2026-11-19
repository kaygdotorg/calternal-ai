import { getContext, setContext } from "svelte";
import type { ActionUi } from "./actions.svelte";
import type { HostSession } from "../wire/session";

/** What every drawn action needs: the dialogs and invoker of the app, and its open session (the view's own contexts). */
export interface ActionContext {
  /** The prompts, pickers and confirmation sheet of the app, and its invoker. */
  ui: ActionUi;
  /** The open session (the view's own activation contexts, the lease). */
  session: HostSession;
  /** The host-issued activation context for a view-level action (a form's submit, a button, a header action), read reactively: renewals replace the view's contexts, and a control drawn while its context was missing enables when it arrives. */
  viewContext(actionId: string): string | undefined;
  /** A shell action the shell should perform (already checked against the allow-list). */
  onshell?: (action: unknown) => void;
}
/** The context key under which an app provides its `ActionContext`. */
export const ACTIONS_KEY = Symbol("description-actions");
/** Provides the app's actions to everything drawn inside it. */
export const provideActions = (c: ActionContext) => setContext(ACTIONS_KEY, c);
/** The app's actions (the dialogs and invoker, the open session). Call while a component initialises. */
export const useActions = (): ActionContext => getContext<ActionContext>(ACTIONS_KEY);

/** Reads one host-authorized row image by its ref (`query.row_images`) as an object URL from the app's bounded cache; null when the host has none. `signal` cancels it. */
export type ImageReader = (ref: string, signal?: AbortSignal) => Promise<string | null>;
/** The context key under which an app provides its `ImageReader`. */
export const IMAGES_KEY = Symbol("description-images");

import type { WindowStore } from "../windows/window-store.svelte";
import type { Selection } from "../windows/selection.svelte";
import type { Component } from "svelte";
/** The stores and selections of the open app, for parts that reach them by bind id (child rows, multiple selection) and the host components the page supplies. */
export interface AppContext {
  /** The window store of a bind (per parent ref for a child bind), made on first use. */
  storeFor(bind: string, parent?: string): WindowStore | undefined;
  /** The selection of a bind, shared by every place that draws it. */
  selectionFor(bind: string): Selection;
  /** The icon the bind's kind declares, for rows whose icon is `kindIcon`. */
  kindIconFor(bind: string): string | undefined;
  /** The route parameters now in force (selectors, the open item). */
  route: Record<string, string>;
  /** Sets a route parameter; binds that read it start over. */
  setRoute(name: string, value: string): void;
  /** The host components, by type: the package's own, overridden by the page's. */
  components: Record<string, Component<{ config: unknown; id: string }>>;
  /** The shell's own controls: a shell action (route, reload, sign out, sheet) and the app switcher. */
  onshell?: (action: unknown) => void;
  /** The page's app switcher: open the app named by a footer entry. */
  openApp?: (id: string) => void;
}
/** The context key under which an app provides its `AppContext`. */
export const APP_KEY = Symbol("description-app");
/** The open app (stores, selections, route, host components), or undefined outside one. Call while a component initialises. */
export const useApp = (): AppContext => getContext<AppContext>(APP_KEY);
