/** Contacts, for the mockup's seam (see ../registry.ts): its collections are the sidebar's groups, its page is the A to Z list and a person's page, and the panel holds groups and what is elsewhere. Fixture only. */
import type { MockupApp } from "../registry";
import { COLLS } from "../../data/collections";
import "./data";
import ContactRoute from "./ContactRoute.svelte";
import { startContact } from "./new";
import ContactPage from "./ContactPage.svelte";
import ContactPanel from "./ContactPanel.svelte";
import ContactLook from "./ContactLook.svelte";

const SIDEBAR = ["ct-fav", "ct-family", "ct-work", "ct-neighbours", "ct-services", "ct-birthdays"];

const contacts: MockupApp = {
  id: "contacts", name: "Contacts", icon: "user", kinds: ["contact"], collections: [...SIDEBAR, "ct-all"].map((i) => COLLS[i]!), sidebar: SIDEBAR, action: "New contact",
  page: ContactPage, itemPage: { contact: ContactRoute }, onAction: () => { startContact(); }, panel: ContactPanel, looks: { contact: { card: ContactLook, cell: ContactLook } },
};
export default contacts;
