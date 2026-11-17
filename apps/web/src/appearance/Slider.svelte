<script lang="ts">
  // A labelled range with its current value shown beside the name. The label is tied to the input by id (a label around an <output> would
  // name the output, not the slider).
  let { label, unit = "", min, max, value, onchange }: { label: string; unit?: string; min: number; max: number; value: number; onchange: (v: number) => void } = $props();
  const id = $props.id();
</script>

<div class="slider">
  <label for={id}>{label}</label>
  <output for={id}>{value}{unit}</output>
  <input {id} type="range" {min} {max} {value} oninput={(e) => onchange(+e.currentTarget.value)} />
</div>

<style>
  .slider { display: grid; grid-template-columns: 1fr auto; gap: var(--s-1) var(--s-4); align-items: center; }
  label { font-weight: var(--strong-weight); }
  output { color: var(--ink-3); font-variant-numeric: tabular-nums; }
  input { grid-column: 1 / -1; width: 100%; accent-color: var(--accent-base); min-height: var(--s-8); }
</style>
