<!--
    Top-level view switcher: the Go/No-Go safety board vs. the generic
    topic-echo view. The AppBar, update banner, and the mandatory safety
    strip live ABOVE this and stay visible on every tab.
-->
<script lang="ts">
    export type TabId = 'board' | 'echo' | 'details';

    interface Props {
        active: TabId;
        onSelect: (t: TabId) => void;
    }

    const { active, onSelect }: Props = $props();

    const tabs: { id: TabId; label: string }[] = [
        { id: 'board', label: 'Go / No-Go board' },
        { id: 'echo', label: 'Topic echo' },
        { id: 'details', label: 'Details' },
    ];
</script>

<nav class="tabbar" aria-label="Views">
    {#each tabs as t (t.id)}
        <button
            type="button"
            class="tab"
            class:active={active === t.id}
            aria-current={active === t.id ? 'page' : undefined}
            onclick={() => onSelect(t.id)}>{t.label}</button
        >
    {/each}
</nav>
