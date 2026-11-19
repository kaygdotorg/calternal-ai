import { getContext, setContext } from "svelte";
import type { ActionUi } from "./actions.svelte";
import type { HostSession } from "../wire/session";

/** What every drawn action needs: the dialogs and invoker of the app, and its open session (the view's own contexts). */
export interface ActionContext { ui: ActionUi; session: HostSession; onshell?: (action: unknown) => void }
export const ACTIONS_KEY = Symbol("description-actions");
export const provideActions = (c: ActionContext) => setContext(ACTIONS_KEY, c);
export const useActions = (): ActionContext => getContext<ActionContext>(ACTIONS_KEY);
