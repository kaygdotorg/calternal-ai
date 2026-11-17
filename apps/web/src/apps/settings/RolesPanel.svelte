<script lang="ts">
  // The space roles and what each allows, read from the server (`roles.list`): a role is a set of capabilities.
  import * as people from "../../server/people";
  import { say } from "../../server/say";
  import "./panel.css";
  let roles = $state<people.RoleInfo[]>([]), error = $state("");
  people.roles().then((r) => (roles = r), (e) => (error = say(e, "load the roles")));
</script>

<div class="sp">
  {#each roles as r (r.name)}<div class="r" data-setting="space.role.{r.name}"><span class="n"><span class="t">{r.name[0]!.toUpperCase() + r.name.slice(1)}</span><span class="h">{r.capabilities.length ? r.capabilities.join(", ") : "Can read what is shared"}</span></span></div>{/each}
  {#if error}<p class="e" role="alert">{error}</p>{/if}
</div>
