<!--
    Always-visible overall banner. Redundant with the verdict stamp on
    purpose — it reads first, before the eye parses the rotated stamp.
    Maps the overall state to a fixed headline + description, and shows
    the "N/M topics live" tag on the right.
-->
<script lang="ts">
    import type { OverallState } from '../types';

    interface Props {
        state: OverallState;
        /** Right-hand tag, e.g. "5/7 topics live". */
        tag: string;
    }

    const { state, tag }: Props = $props();

    const COPY: Record<OverallState, { h: string; d: string }> = {
        fault: {
            h: 'FAULT',
            d: 'Active safety fault — the car must not move.',
        },
        hold: {
            h: 'STALE HEARTBEAT',
            d: 'A live topic went silent past the watchdog — check the link.',
        },
        go: {
            h: 'NOMINAL',
            d: 'All monitored safety signals healthy and fresh.',
        },
        standby: {
            h: 'Waiting for data…',
            d: 'No topics received yet.',
        },
    };

    const copy = $derived(COPY[state]);
    // Only the three colored states get a class; standby is the bare banner.
    const stateClass = $derived(state === 'standby' ? '' : state);
</script>

<section
    class="overall {stateClass}"
    role="status"
    aria-live="assertive"
>
    <span class="beacon"></span>
    <div class="otxt">
        <span class="ohead">{copy.h}</span>
        <span class="odesc">{copy.d}</span>
    </div>
    <span class="otag">{tag}</span>
</section>
