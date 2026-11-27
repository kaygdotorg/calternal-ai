// Chat registers like any app: its page is the conversation view (Conversation.svelte), which also draws a chat item opened in a window of its own. Its sidebar collections are declared in data/collections.ts (they are the Groups, People, Cals, Harnesses and Archived the earlier Chat interface had).
import { DEFAULT_SIDEBARS, ITEMS } from "../../data/collections";
import type { MockupApp } from "../registry";
import Page from "./Page.svelte";
import Conversation from "./Conversation.svelte";
export default { id: "chat", name: "Chat", icon: "chat", kinds: ["chat"], collections: [], sidebar: DEFAULT_SIDEBARS.chat!, title: (w) => ITEMS[w.sel ?? "c-audit"]?.title, page: Page, itemPage: { chat: Conversation } } satisfies MockupApp;
