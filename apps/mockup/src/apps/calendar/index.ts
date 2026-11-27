/** Calendar, for the mockup's seam (see ../registry.ts): its collections are the sidebar, its page is month, week, day and agenda, an event opens as a page, and the panel holds the month and the calendar switches. Fixture only. */
import type { MockupApp } from "../registry";
import { COLLS } from "../../data/collections";
import "./data";
import CalendarPage from "./CalendarPage.svelte";
import CalPanel from "./CalPanel.svelte";
import EventRoute from "./EventRoute.svelte";
import { startEvent } from "./new";
import EventItem from "./EventItem.svelte";

const SIDEBAR = ["cal-today", "cal-upcoming", "cal-personal", "cal-work", "cal-family", "cal-birthdays"];

const calendar: MockupApp = {
  id: "calendar", name: "Calendar", icon: "calendar", kinds: ["event"], collections: SIDEBAR.map((i) => COLLS[i]!), sidebar: SIDEBAR, action: "New event",
  page: CalendarPage, itemPage: { event: EventRoute }, onAction: () => { startEvent(); }, panel: CalPanel, looks: { event: { card: EventItem, cell: EventItem } },
};
export default calendar;
