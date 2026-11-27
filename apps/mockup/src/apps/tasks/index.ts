/** The Tasks mockup app: fixture tasks, its sidebar as collections, its window page (a list page), the task page, the right panel, and the task's looks (row with inline checkbox, card) which the shell uses wherever a task is drawn. Registered by `../index.ts`. */
import type { MockupApp } from "../registry";
import { collections } from "./data";
import TasksPage from "./TasksPage.svelte";
import TaskPage from "./TaskPage.svelte";
import TaskPanel from "./TaskPanel.svelte";
import TaskRow from "./TaskRow.svelte";
import TaskCard from "./TaskCard.svelte";

const app: MockupApp = {
  id: "tasks", name: "Tasks", icon: "tasks", kinds: ["task"], action: "New task",
  collections, sidebar: ["tk-smart", "tk-today", "tk-projects"],
  page: TasksPage, itemPage: { task: TaskPage }, panel: TaskPanel, looks: { task: { row: TaskRow, card: TaskCard, cell: TaskCard } },
};
export default app;
