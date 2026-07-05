<!--
    RES-holder heads-up / kill-decision fullscreen (#57).

    A glanceable, high-contrast, distance-readable safety verdict for the RES
    holder during an autonomous run: one giant word (SAFE / HOLD / STOP), the AS
    state, and the blocking reason — no chrome to hunt through when the decision
    is split-second. A vanished link overrides everything ("LINK LOST — do not
    trust"). Esc or click-anywhere exits.
-->
<script lang="ts">
    import type { OverallState } from '../types';

    interface Props {
        state: OverallState;
        reason: string;
        asWord: string | null;
        linkLost: boolean;
        onClose: () => void;
    }
    const { state, reason, asWord, linkLost, onClose }: Props = $props();

    // Big verdict word + severity class, keyed off the board's overall state.
    const verdict = $derived.by(() => {
        if (linkLost) return { word: 'LINK LOST', cls: 'kv-fault', sub: 'link down — do not trust this reading' };
        switch (state) {
            case 'fault':
                return { word: 'STOP', cls: 'kv-fault', sub: reason };
            case 'go':
                return { word: 'SAFE', cls: 'kv-go', sub: reason };
            case 'hold':
                return { word: 'HOLD', cls: 'kv-hold', sub: reason };
            default:
                return { word: 'STANDBY', cls: 'kv-standby', sub: reason };
        }
    });

    function onKey(e: KeyboardEvent): void {
        if (e.key === 'Escape') onClose();
    }
</script>

<svelte:window onkeydown={onKey} />

<div
    class="killview {verdict.cls}"
    role="button"
    tabindex="0"
    aria-label="Exit RES view"
    onclick={onClose}
    onkeydown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') onClose();
    }}
>
    <div class="kv-word">{verdict.word}</div>
    {#if asWord}<div class="kv-as">{asWord}</div>{/if}
    {#if verdict.sub}<div class="kv-sub">{verdict.sub}</div>{/if}
    <div class="kv-exit">Esc / click to exit</div>
</div>
