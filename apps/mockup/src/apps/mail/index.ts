/** Mail, for the mockup's seam (see ../registry.ts): its collections are the sidebar, its page is the open conversation, and compose is a window beside. Fixture only. */
import type { MockupApp } from "../registry";
import { COLLS } from "../../data/collections";
import "./data";
import MailItem from "./MailItem.svelte";
import MailPage from "./MailPage.svelte";
import MailPanel from "./MailPanel.svelte";
import MailLook from "./MailLook.svelte";
import { startDraft } from "./compose";

const SIDEBAR = ["mail-inbox", "mail-flagged", "mail-drafts", "mail-sent", "mail-personal", "mail-studio", "mail-receipts", "mail-travel"];
const mail: MockupApp = {
  id: "mail", name: "Mail", icon: "mail", kinds: ["mail"], collections: SIDEBAR.map((i) => COLLS[i]!), sidebar: SIDEBAR, action: "Write",
  page: MailPage, itemPage: { mail: MailItem }, panel: MailPanel, onAction: () => { startDraft("new"); },
  /** Cards and cells (boards, galleries); rows keep the shared one-line row, which the sidebar and drag rely on. */ looks: { mail: { card: MailLook, cell: MailLook } },
};
export default mail;
