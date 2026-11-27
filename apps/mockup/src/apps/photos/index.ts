/** The Photos app of the mockup: its collections (Library, Recently added, Albums, Favourite photos), sidebar, page, photo page and side panel. See `README.md` of the mockup for the seam. */
import type { MockupApp } from "../registry";
import Look from "../Look.svelte";
import { COLLS } from "../../data/collections";
import "./data";
import PhotosPage from "./PhotosPage.svelte";
import PhotoItemPage from "./PhotoItemPage.svelte";
import { albumOf, listOf, metaOf } from "./data";
import { ITEMS } from "../../data/collections";
import PhotosPanel from "./PhotosPanel.svelte";

const ids = ["p-lib", "p-recent", "p-albums", "p-favs"];
export const photos: MockupApp = {
  id: "photos", name: "Photos", icon: "image", kinds: ["photo"],
  collections: ids.map((id) => COLLS[id]!), sidebar: ["p-lib", "p-recent", "p-albums"],
  title: (w) => (w.sel ? (albumOf(w.sel)?.title ?? listOf(w.sel)?.title ?? (metaOf(w.sel) ? ITEMS[w.sel]?.title : undefined)) : undefined), page: PhotosPage, itemPage: { photo: PhotoItemPage }, looks: { photo: Look }, panel: PhotosPanel,
};
