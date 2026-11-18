<script lang="ts">
  // The Share sheet (the sharing grill, ported from the existing app's ShareSheet): the item's title, one plain sentence on who can see it, "Add people or groups" above the list, each person or group with View / Edit
  // and remove, then the links (name, expiry, QR code, Copy). A phone gets the inset bottom sheet; a desktop gets a popover that grows out of the Share button (the shared menu rule: `Sheet` on touch, `Popover` otherwise).
  // What the caller may change comes from `sharing.list`: `can_manage_audiences` (people and groups) and `can_manage_links` (links need a Manager or the Owner unless the space lets Editors); a part the caller may not
  // change is not drawn as a control, and a line says who can.
  import { onMount, tick } from "svelte";
  import { Avatar, Button, Icon, IconButton, Popover, SearchField, Selector, Sheet, currentForm, failed, toast } from "@calternal/ui";
  import { say } from "../../../server/say";
  import { qrSvg } from "../../../lib/qr";
  import { EXPIRIES, audienceKey, expiryText, isGroup, sentenceOf, sharing, isLink, type Found, type ItemRef, type LinkGrant, type NamedGrant, type Permission, type ShareList } from "./sharing";
  let { item, title, trigger, onclose }: { item: ItemRef; title: string; trigger: HTMLElement | null; onclose: () => void } = $props();

  let list = $state<ShareList | null>(null);
  let unreadable = $state(false);
  const sentence = $derived(list ? sentenceOf(list) : "");
  const people = $derived(list?.grants.filter((g): g is NamedGrant => !isLink(g)) ?? []);
  const links = $derived(list?.grants.filter(isLink) ?? []);

  async function read(cursor?: string) {
    try {
      const page = await sharing.list(item, cursor);
      list = cursor && list ? { ...page, grants: [...list.grants, ...page.grants] } : page;
    } catch (e) {
      unreadable = true;
      failed(say(e, "read who can see this"), () => void read());
    }
  }
  onMount(() => void read());
  /** Runs a change, then reads the list again whatever happened (a refused change leaves it as it was). */
  async function change(run: () => Promise<unknown>) {
    try { await run(); } catch (e) { failed(say(e, "change that")); }
    await read();
    // The row that was pressed may be gone (removed): focus goes to the add field, not to the page behind.
    if (!document.activeElement || document.activeElement === document.body) { await tick(); field?.focus(); }
  }

  // ---- Add people and groups
  let field = $state<HTMLInputElement>();
  let query = $state(""), found = $state<Found[]>([]), pickAt = $state(0);
  $effect(() => {
    const q = query.trim();
    if (!q) { found = []; return; }
    const h = setTimeout(() => void sharing.audiences(item, q).then((r) => { found = r.audiences.slice(0, 5); pickAt = 0; }, () => (found = [])), 150);
    return () => clearTimeout(h);
  });
  function add(f: Found) { query = ""; found = []; void change(() => sharing.grant(item, f.audience, "view")); }
  function addKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); pickAt = Math.max(0, Math.min(found.length - 1, pickAt + (e.key === "ArrowDown" ? 1 : -1))); }
    else if (e.key === "Enter" && found[pickAt]) { e.preventDefault(); add(found[pickAt]!); }
  }

  // ---- Links
  const DAY = 86_400_000;
  /** The expiry choice a link's date stands for: its nearest option, or none when it is already past. */
  const choiceOf = (at: number | null): string | null => (at === null ? "never" : at <= Date.now() ? null : String(EXPIRIES.slice(1).reduce((best, e) => (Math.abs(e.days! * DAY - (at - Date.now())) < Math.abs(best * DAY - (at - Date.now())) ? e.days! : best), 1)));
  const expiryOptions = EXPIRIES.map((e) => ({ value: e.days === null ? "never" : String(e.days), label: e.label }));
  const setExpiry = (g: LinkGrant, v: string) => void change(() => sharing.link_update(item, g.grant_id, undefined, v === "never" ? null : Date.now() + Number(v) * DAY));
  const createLink = () => void change(() => sharing.link_create(item, `Link ${links.length + 1}`));
  // Renaming a link: its name is a field right in the row (a tap on it is a real focus, so a phone's keyboard brings it into view; a field focused by code is not scrolled to on iOS). Enter or leaving keeps the name; Escape
  // puts the old one back; an empty name is put back too (a link needs a name).
  function rename(g: LinkGrant, el: HTMLInputElement) {
    const name = el.value.trim();
    if (!name) el.value = g.label;
    else if (name !== g.label) void change(() => sharing.link_update(item, g.grant_id, name));
  }
  let qr = $state<{ id: string; svg: string } | null>(null);
  async function copy(g: LinkGrant) {
    try {
      const { url } = await sharing.link_copy(item, g.grant_id);
      await navigator.clipboard.writeText(url);
      toast("Link copied");
    } catch (e) { failed(say(e, "copy the link")); }
  }
  async function showQr(g: LinkGrant) {
    if (qr?.id === g.grant_id) { qr = null; return; }
    try { qr = { id: g.grant_id, svg: qrSvg((await sharing.link_copy(item, g.grant_id)).url) }; } catch (e) { failed(say(e, "make the code")); }
  }
</script>

{#snippet body()}
  <div class="share" data-share>
    <h2 class="head">{title}</h2>
    {#if list}
      <p class="who" data-share-sentence>{sentence}</p>
      {#if list.can_manage_audiences}
        <SearchField bind:el={field} bind:value={query} label="Add people or groups" onkeydown={addKey} />
        {#if found.length}
          <ul class="found" role="listbox" aria-label="People and groups">
            {#each found as f, i (audienceKey(f.audience))}
              <li role="option" aria-selected={i === pickAt}><button type="button" class="pick" class:on={i === pickAt} onclick={() => add(f)}><Avatar name={f.name} /><span class="nm">{f.name}</span>{#if isGroup(f.audience)}<small>Group</small>{/if}</button></li>
            {/each}
          </ul>
        {:else if query.trim()}<p class="none">Nobody matches “{query.trim()}”.</p>{/if}
      {:else}
        <p class="none">Only an Editor, a Manager or the Owner can change who has access.</p>
      {/if}
      {#each people as g (g.grant_id)}
        <div class="person" data-person={g.grant_id}>
          <Avatar name={g.name} />
          <span class="nm">{g.name}{#if isGroup(g.audience)} <small>Group</small>{/if}</span>
          {#if list.can_manage_audiences}
            <Selector size="sm" fill={false} label="{g.name}: what they can do" value={g.permission} options={[{ value: "view", label: "View" }, { value: "edit", label: "Edit" }]} onchange={(v: Permission) => void change(() => sharing.grant(item, g.audience, v))} />
            <IconButton small icon="close" label="Remove {g.name}" onclick={() => void change(() => sharing.revoke(item, g.grant_id))} />
          {:else}<span class="role">{g.permission === "edit" ? "Can edit" : "Can view"}</span>{/if}
        </div>
      {/each}
      {#if list.next_cursor}<Button variant="quiet" text="Show more" onclick={() => void read(list!.next_cursor)} />{/if}
      {#if links.length || list.can_manage_links}
        <p class="sec">Links</p>
        {#each links as g (g.grant_id)}
          <div class="link" data-link={g.grant_id}>
            <div class="l1">
              <Icon name="link" />
              <span class="nm">
                {#if list.can_manage_links}<input class="ren" aria-label="Link name" value={g.label} onblur={(e) => rename(g, e.currentTarget)} onkeydown={(e) => { if (e.key === "Enter") e.currentTarget.blur(); else if (e.key === "Escape") { e.stopPropagation(); e.currentTarget.value = g.label; e.currentTarget.blur(); } }} />{:else}{g.label}{/if}
                <small>Anyone with it can view · {g.expires_at === null ? "Never expires" : expiryText(g.expires_at)}</small>
              </span>
              {#if list.can_manage_links}<IconButton small icon="close" label="Remove {g.label}" onclick={() => void change(() => sharing.revoke(item, g.grant_id))} />{/if}
            </div>
            {#if list.can_manage_links}
              <Selector size="sm" label="{g.label}: expires" value={choiceOf(g.expires_at)} options={expiryOptions} onchange={(v: string) => setExpiry(g, v)} />
              <div class="l3"><Button variant="secondary" text={qr?.id === g.grant_id ? "Hide code" : "QR code"} onclick={() => void showQr(g)} /><Button variant="primary" icon="copy" text="Copy link" onclick={() => void copy(g)} /></div>
              {#if qr?.id === g.grant_id}<div class="qr" role="img" aria-label="QR code for {g.label}">{@html qr.svg}</div>{/if}
            {/if}
          </div>
        {/each}
        {#if list.can_manage_links}<div class="make"><Button variant="secondary" icon="plus" text="Create a link" onclick={createLink} /></div>
        {:else}<p class="none">Links need a Manager or the Owner.</p>{/if}
      {:else if list.can_manage_audiences}
        <p class="none">Links need a Manager or the Owner.</p>
      {/if}
    {:else if unreadable}<p class="none">Couldn't read who can see this.</p>{/if}
  </div>
{/snippet}

{#if currentForm() === "touch"}
  <Sheet open label="Share" {onclose}>{@render body()}</Sheet>
{:else}
  <Popover open anchor={trigger} {trigger} label="Share" role="dialog" prefer="bottom" morph {onclose}>{@render body()}</Popover>
{/if}

<style>
  .share { display: grid; gap: var(--s-3); padding: var(--s-5); width: min(26rem, 100%); box-sizing: border-box; align-content: start; }
  .head { margin: 0; padding: 0 var(--s-2); font-size: var(--text-heading); font-weight: var(--strong-weight); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .who { margin: 0 0 var(--s-2); padding: 0 var(--s-2); color: var(--ink-2); font-size: var(--text-ui); line-height: 1.4; }
  .sec { margin: var(--s-3) 0 0; padding: 0 var(--s-2); font-size: var(--text-label); font-weight: var(--strong-weight); color: var(--ink-3); }
  .none { margin: 0; padding: 0 var(--s-2); color: var(--ink-3); font-size: var(--text-label); }
  .found { list-style: none; margin: 0; padding: 0; display: grid; gap: var(--s-1); }
  .pick, .person { display: flex; align-items: center; gap: var(--s-4); width: 100%; min-height: var(--row-h); padding: 0 var(--s-3); border-radius: var(--r-select); text-align: start; }
  .pick.on { background: var(--accent-soft); }
  @media (hover: hover) and (pointer: fine) { .pick:hover { background: var(--hover); } }
  .nm { flex: 1; min-width: 0; display: grid; font-size: var(--text-ui); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  small, .role { font-size: var(--text-label); color: var(--ink-3); }
  .role { flex: none; }
  .link { display: grid; gap: var(--s-3); padding: var(--s-3); border-radius: var(--r-select); background: rgb(var(--ink-rgb) / 0.05); }
  .l1 { display: flex; align-items: center; gap: var(--s-4); color: var(--ink-2); }
  .l1 .nm { color: var(--ink); }
  .ren { width: 100%; min-width: 0; margin: 0 calc(var(--s-2) * -1); padding: 0 var(--s-2); min-height: var(--row-h); border: 0; border-radius: var(--r-select); background: none; color: var(--ink); font-size: var(--text-ui); text-align: start; }
  .ren:focus, .ren:focus-visible { outline: none; background: rgb(var(--ink-rgb) / 0.07); }
  .l3 { display: flex; flex-wrap: wrap; gap: var(--s-2); }
  .qr { justify-self: start; width: 9rem; padding: var(--s-2); border-radius: var(--r-select); background: #fff; line-height: 0; }
  .qr :global(svg) { width: 100%; height: auto; }
  .make { justify-self: start; }
</style>
