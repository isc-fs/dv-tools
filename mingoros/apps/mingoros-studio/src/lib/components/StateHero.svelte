<!--
    Hero banner: the dominant AS-state readout on the left with its
    status rail, and the "bench verdict" inspection stamp on
    the right (READY TO DRIVE / NOT READY / FAULT / STANDBY with the
    spelled-out blocking interlocks). Theming class comes from the
    verdict state; the stamp call shrinks when the word is long.
-->
<script lang="ts">
    import type { OverallState, TopicSnapshot } from '../types';
    import { assiLook, extractWord, fmtAge } from '../model';

    interface Props {
        /** Verdict state — themes the whole banner. */
        state: OverallState;
        /** The /assi/state snapshot (may be undefined / not-ok). */
        asRow: TopicSnapshot | undefined;
        /** Stamp "why" line — spelled-out blockers / pending list. */
        reason: string;
    }

    const { state, asRow, reason }: Props = $props();

    const stateClass = $derived(state === 'standby' ? '' : state);
    const ok = $derived(asRow != null && asRow.state === 'ok');

    // AS headline word: "AS_READY" out of "data: 1 (AS_READY)".
    const word = $derived(
        ok && asRow ? extractWord(asRow.value) || asRow.value || '— — —' : '— — —',
    );

    // The car's ASSI light (yellow/blue/off + flashing) for this AS state, so
    // the app mirrors the physical indicator. Off/grey when there's no data.
    const assi = $derived(assiLook(ok ? word : null));

    // enum number pulled from the "data: N" prefix, for the raw line.
    const enumNum = $derived.by<string | null>(() => {
        if (!ok || !asRow || !asRow.value) return null;
        const m = asRow.value.match(/data:\s*(-?\d+)/i);
        return m ? m[1] : '?';
    });

    const stale = $derived(ok && asRow != null && asRow.fresh === false);
    const ageText = $derived(
        ok && asRow ? fmtAge(asRow.age_ms) + (stale ? ' · STALE' : '') : '',
    );

    // Stamp call word + its size (source shrinks anything over 9 chars).
    const call = $derived(
        state === 'fault'
            ? 'FAULT'
            : state === 'go'
              ? 'READY TO DRIVE'
              : state === 'hold'
                ? 'NOT READY'
                : 'STANDBY',
    );
    const callSize = $derived(call.length > 9 ? '23px' : '32px');
</script>

<section class="verdict {stateClass}" aria-live="polite">
    <div class="as assi-{assi.color}" class:assi-blink={assi.blink}>
        <span class="rail"></span>
        <div class="eyebrow">
            <span>Autonomous system · /assi/state</span>
            <span class="age" class:stale>{ageText}</span>
        </div>
        <div class="word-row">
            <span
                class="assi-led"
                title="ASSI — mirrors the car's Autonomous System Status Indicator light"
            ></span>
            <div class="word">{word}</div>
        </div>
        <div class="raw">
            {#if ok && asRow}
                enum <b>{enumNum}</b> · {asRow.value}
            {:else if asRow != null && asRow.state === 'unavailable'}
                backend could not subscribe to /assi/state
            {:else}
                waiting for /assi/state
            {/if}
        </div>
    </div>
    <div class="stamp-wrap">
        <div class="stamp">
            <div class="label">Bench verdict</div>
            <div class="call" style:font-size={callSize}>{call}</div>
            <div class="why">{reason}</div>
        </div>
    </div>
</section>
