<!--
    Details tab — the reference data that doesn't need to be on the glance-read
    Go/No-Go board: the session debrief and the full raw-topic table. Moved here
    so the board itself fits one viewport with no scrolling.
-->
<script lang="ts">
    import type { Meta, TopicSnapshot } from '../types';
    import RawTopics from './RawTopics.svelte';
    import SessionDebrief from './SessionDebrief.svelte';

    interface Props {
        rows: TopicSnapshot[];
        meta: Meta;
    }
    const { rows, meta }: Props = $props();
</script>

<section class="details">
    <SessionDebrief />
    <RawTopics {rows} {meta} />
</section>
