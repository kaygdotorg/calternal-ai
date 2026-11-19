import type { AppDescription, ManifestV2 } from "../generated/author-types.js";
import { ContractValidationError } from "./index.js";

type App = ManifestV2["apps"][number];
type Kind = ManifestV2["kinds"][number];
type Operation = ManifestV2["operations"][number];
type Bind = NonNullable<AppDescription["binds"]>[number];

type Row = NonNullable<Bind["row"]>;
type BindInput = Omit<Bind, "row"> & {
  row?: Partial<Omit<Row, "icon">> & { icon?: Row["icon"] | false };
};
type ViewInput = NonNullable<AppDescription["view"]> & { binds?: BindInput[] };
type ChromeInput = NonNullable<AppDescription["chrome"]> & { binds?: BindInput[] };

type AuthoredApp = {
  metadata: Omit<App, "description">;
  template: AppDescription;
};

type PluginInput = Omit<ManifestV2, "apps" | "worker" | "network" | "operations"> & {
  apps: AuthoredApp[];
  network?: ManifestV2["network"];
  operations?: Operation[];
};

type AuthoredPlugin = {
  metadata: Omit<ManifestV2, "apps" | "worker">;
  apps: AuthoredApp[];
};

/**
 * Assemble v2 declarations for the offline package build. Adds own kinds to
 * reads and defaults to no network and no worker operations. Performs no I/O.
 */
export function definePlugin({ apps, network = "none", operations = [], ...metadata }: PluginInput): AuthoredPlugin {
  return {
    apps,
    metadata: {
      ...metadata,
      network,
      operations,
      reads: [...new Set([...metadata.kinds.map(kind => kind.id), ...metadata.reads ?? []])],
    },
  };
}

/**
 * Give a kind a generated title mapping. The selected field must explicitly
 * declare a string in the inline metadata schema. Fails at authoring time
 * if that field is missing or has another type.
 */
export function defineKind({ title_field = "title", ...kind }: Omit<Kind, "title_field"> & {
  title_field?: Kind["title_field"];
}): Kind {
  const schema = kind.schema;
  const properties = schema && typeof schema === "object" && "properties" in schema
    ? schema.properties
    : undefined;
  const title = properties && typeof properties === "object" && Object.hasOwn(properties, title_field)
    ? (properties as Record<string, unknown>)[title_field]
    : undefined;
  if (!title || typeof title !== "object" || !("type" in title) || title.type !== "string") {
    throw new ContractValidationError(`Kind ${kind.id}: title_field ${title_field} needs a declared string property.`);
  }
  return { ...kind, title_field };
}

/** Declare an operation using the sealed manifest type. The build checks its value constraints. */
export function defineOperation(operation: Operation): Operation {
  return operation;
}

function defaultBind(bind: BindInput): Bind {
  const search = bind.source.type === "search";
  const { icon = { kindIcon: true as const }, ...row } = bind.row ?? {};
  const title = row.title ?? { field: "title" };
  return {
    ...bind,
    fields: [...new Set([title.field, ...bind.fields ?? []])],
    window: { size: 60, count: search ? "none" : "exact", ...bind.window },
    row: { ...row, title, icon: icon === false ? undefined : icon },
    states: {
      empty: { title: search ? "No results" : "Nothing here yet" },
      loading: { title: "Loading" },
      error: { title: "Couldn’t load items" },
      ...bind.states,
    },
  };
}

/** Build view content and its Binds. Defaults each Bind independently in linear time. */
export function defineView({ binds, ...view }: ViewInput): {
  /** Content rendered in the app's main area. */
  view: NonNullable<AppDescription["view"]>;
  /** Queries this view contributes to the app's shared Bind list. */
  binds: Bind[];
} {
  return { view, binds: (binds ?? []).map(defaultBind) };
}

/** Build chrome and its Binds using the same defaults as view content. */
export function defineChrome({ binds, ...chrome }: ChromeInput): {
  /** Controls rendered around the app content. */
  chrome: NonNullable<AppDescription["chrome"]>;
  /** Queries these controls contribute to the app's shared Bind list. */
  binds: Bind[];
} {
  return { chrome, binds: (binds ?? []).map(defaultBind) };
}

/**
 * Assemble one app template. Chrome and content reference the same Bind IDs.
 * The build supplies the template path and digest after validating it.
 */
export function defineApp({ view, chrome, revision = 1, ...metadata }: Omit<App, "description"> & {
  view?: ReturnType<typeof defineView>;
  chrome?: ReturnType<typeof defineChrome>;
  revision?: AppDescription["revision"];
  requires?: AppDescription["requires"];
  cache?: AppDescription["cache"];
}): AuthoredApp {
  const { requires, cache, ...app } = metadata;
  const binds = [...view?.binds ?? [], ...chrome?.binds ?? []];
  return {
    metadata: app,
    template: {
      version: 2, kind: "app", revision, requires, cache, binds,
      view: view?.view,
      chrome: chrome?.chrome,
    },
  };
}
