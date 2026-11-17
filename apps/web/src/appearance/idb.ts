/** A tiny IndexedDB key-value helper for the fixture libraries: stores `w` (wallpapers) and `f` (own font files). Fixture-only; the server stores these. */
const DB = "cal-appearance";

const open = (): Promise<IDBDatabase> =>
  new Promise((res, rej) => {
    const r = indexedDB.open(DB, 1);
    r.onupgradeneeded = () => {
      r.result.createObjectStore("w");
      r.result.createObjectStore("f");
    };
    r.onsuccess = () => res(r.result);
    r.onerror = () => rej(r.error);
  });

/** One request against a store. */
export async function tx<T>(store: "w" | "f", mode: IDBTransactionMode, f: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const db = await open();
  return new Promise((res, rej) => {
    const r = f(db.transaction(store, mode).objectStore(store));
    r.onsuccess = () => res(r.result);
    r.onerror = () => rej(r.error);
  });
}

/** Every value in a store. */
export async function all<T>(store: "w" | "f"): Promise<T[]> {
  const keys = (await tx(store, "readonly", (s) => s.getAllKeys())) as string[];
  return Promise.all(keys.map((k) => tx<T>(store, "readonly", (s) => s.get(k))));
}
