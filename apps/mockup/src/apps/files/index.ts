/** The Files app of the mockup: its collections (Recent, Shared with me, Favourites, Folders), sidebar, page, file page and side panel. See `README.md` of the mockup for the seam. */
import type { MockupApp } from "../registry";
import Look from "../Look.svelte";
import { COLLS } from "../../data/collections";
import "./data";
import FilesPage from "./FilesPage.svelte";
import FileItemPage from "./FileItemPage.svelte";
import { metaOf } from "./data";
import FilesPanel from "./FilesPanel.svelte";

export const files: MockupApp = {
  id: "files", name: "Files", icon: "files", kinds: ["file"], action: "Upload",
  collections: ["f-recent", "f-shared", "f-fav", "f-top"].map((id) => COLLS[id]!), sidebar: ["f-recent", "f-shared", "f-fav", "f-top"],
  title: (w) => (w.sel ? metaOf(w.sel)?.name : undefined), page: FilesPage, itemPage: { file: FileItemPage }, looks: { file: Look }, panel: FilesPanel,
};
