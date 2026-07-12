<!--
    Startup tab (#feat-59) — live, read-only startup guide.

    Reads the same /debug-parsed signals + AS-state word the board uses and tells
    the operator which step of the safe power-up sequence they're on and what to
    do next (LVMS → ASMS → TSMS → EBS self-check → RES → mission → READY → 5 s
    dwell → GO). Ports the uDV startup-trainer's checklist + hint engine, driven
    live instead of by simulator toggles. It NEVER commands anything — indicators
    only.
-->
<script lang="ts">
    import { onMount } from 'svelte';
    import type { ParsedSignal } from '../types';
    import { deriveStartup } from '../startup';
    import PneumaticDiagram from './PneumaticDiagram.svelte';

    interface Props {
        signalMap: Record<string, ParsedSignal>;
        asWord: string | null;
        /** live fresh data flowing (= uDV powered) */
        receiving: boolean;
        /** decoded /ami/mission word */
        missionWord: string | null;
        /** DV pipeline (/dv/status) up */
        dvpcUp: boolean;
    }
    const { signalMap, asWord, receiving, missionWord, dvpcUp }: Props = $props();

    // Time the mandatory 5 s READY dwell locally (the wire doesn't carry it),
    // ticking a `now` clock so the countdown updates smoothly.
    let readyAt = $state<number>(0);
    let now = $state<number>(Date.now());

    const isReadyState = $derived(
        (asWord || '').toUpperCase().includes('READY') &&
            !(asWord || '').toUpperCase().includes('DRIVING'),
    );
    $effect(() => {
        if (isReadyState) {
            if (readyAt === 0) readyAt = Date.now();
        } else {
            readyAt = 0;
        }
    });
    const readyElapsedS = $derived(readyAt ? (now - readyAt) / 1000 : 0);

    onMount(() => {
        const id = setInterval(() => (now = Date.now()), 100);
        return () => clearInterval(id);
    });

    const view = $derived(
        deriveStartup({ signalMap, asWord, receiving, missionWord, dvpcUp, readyElapsedS }),
    );

    const glyph = (s: string): string =>
        s === 'done' ? '✓' : s === 'err' ? '!' : s === 'current' ? '…' : '';
</script>

<section class="startup">
    <!-- live AS-state + ASSI lamp + power-up indicators -->
    <div class="su-cluster su-phase-{view.phase}">
        <div class="su-lamp assi-{view.assi.color}" class:blink={view.assi.blink}></div>
        <div class="su-as">
            <div class="su-asword">{view.receiving ? view.asWord ?? '—' : 'NO DATA'}</div>
            <div class="su-assicap">
                {view.receiving ? 'live · AS state from /assi/state' : 'no uDV heartbeat'}
            </div>
        </div>
        <div class="su-power">
            {#each view.power as p (p.id)}
                <div class="su-led su-{p.tone}" title={p.label + ': ' + p.val}>
                    <span class="su-dot"></span>{p.label}<span class="su-val">{p.val}</span>
                </div>
            {/each}
        </div>
    </div>

    <!-- EBS self-check: sub-state rail (state-only; the wire carries the FSM
         state, not raw tank pressures) -->
    <div class="su-ebs su-ebs-{view.ebs.tone}">
        <div class="su-ebs-head">
            <span class="su-ebs-k">EBS self-check</span>
            <span class="su-ebs-v">{view.ebs.label}</span>
            <span class="su-ebs-note">runs in AS OFF · FS-Rules T15</span>
        </div>
        <div class="su-rail">
            {#each view.ebs.rail as n, i (n.key)}
                {#if i > 0}
                    <span
                        class="su-rline su-r-{view.ebs.rail[i - 1].state === 'done'
                            ? 'done'
                            : n.state === 'err'
                              ? 'err'
                              : 'pending'}"
                    ></span>
                {/if}
                <span class="su-rnode su-r-{n.state}" title={n.key}>
                    <span class="su-rdot"></span>
                    <span class="su-rlbl">{n.label}</span>
                </span>
            {/each}
        </div>
        <div class="su-diagram">
            <PneumaticDiagram p={view.pneumatic} />
        </div>
        {#if view.ebs.coarse && view.ebs.tone !== 'idle'}
            <div class="su-ebs-coarse">
                firmware reported <code>{view.ebs.raw}</code> — sub-states shown from the pass/fail result (no per-step token on the wire)
            </div>
        {/if}
    </div>

    <!-- the live checklist -->
    <ul class="su-steps">
        {#each view.steps as s (s.id)}
            <li class="su-step su-{s.state}">
                <span class="su-box">{glyph(s.state)}</span>
                <span class="su-lbl">{s.label}</span>
                {#if s.detail}<span class="su-detail">{s.detail}</span>{/if}
            </li>
        {/each}
    </ul>

    <!-- what to do next -->
    <div class="su-hint su-hint-{view.hintTone}">
        <b>{view.phase === 'driving' || view.phase === 'finished' ? 'Done.' : view.phase === 'emergency' || view.phase === 'failed' ? 'Stop.' : 'Next:'}</b>
        {view.hint}
    </div>
</section>

<style>
    /* Two-column guide on wide screens: the AS-state headline spans the top,
       then the step checklist + hint sit beside the EBS self-check visual so
       the width isn't wasted. Collapses to one column when narrow. */
    .startup {
        max-width: 1240px;
        width: 100%;
        margin: 4px auto 0;
        display: grid;
        grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr);
        grid-template-areas:
            'cluster cluster'
            'steps   ebs'
            'hint    ebs';
        gap: 14px;
        align-items: start;
    }
    .su-cluster {
        grid-area: cluster;
    }
    .su-steps {
        grid-area: steps;
    }
    .su-hint {
        grid-area: hint;
    }
    .su-ebs {
        grid-area: ebs;
        align-self: start;
    }
    @media (max-width: 900px) {
        .startup {
            max-width: 760px;
            grid-template-columns: 1fr;
            grid-template-areas:
                'cluster'
                'ebs'
                'steps'
                'hint';
        }
    }

    /* cluster */
    .su-cluster {
        display: grid;
        grid-template-columns: auto 1fr auto;
        align-items: center;
        gap: 18px;
        padding: 18px 20px;
        border: 1px solid var(--line);
        border-radius: 14px;
        background: var(--panel);
    }
    .su-lamp {
        width: 62px;
        height: 62px;
        border-radius: 50%;
        background: #0c1017;
        border: 1px solid var(--line-2);
        box-shadow: inset 0 2px 12px #000;
        flex: none;
    }
    .su-lamp.assi-yellow {
        background: radial-gradient(circle at 42% 38%, #ffe27a, #ffd23f);
        box-shadow: 0 0 30px #ffd23f88, inset 0 0 12px #a8871a;
    }
    .su-lamp.assi-blue {
        background: radial-gradient(circle at 42% 38%, #8dc2ff, #4b9bff);
        box-shadow: 0 0 30px #4b9bff88, inset 0 0 12px #17457d;
    }
    .su-lamp.blink {
        animation: su-blink 0.303s steps(1, end) infinite;
    }
    @keyframes su-blink {
        0%, 49% { opacity: 1; }
        50%, 100% { opacity: 0.12; }
    }
    .su-asword {
        font-size: 26px;
        font-weight: 800;
        letter-spacing: -0.01em;
        line-height: 1.05;
    }
    .su-assicap {
        font-family: var(--mono);
        font-size: 11px;
        color: var(--ink-dim);
        margin-top: 3px;
    }
    .su-power {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 6px;
    }
    .su-led {
        display: flex;
        align-items: center;
        gap: 7px;
        padding: 6px 10px;
        border-radius: 8px;
        background: var(--ground-2);
        border: 1px solid var(--line);
        font-family: var(--mono);
        font-size: 10.5px;
        white-space: nowrap;
    }
    .su-led .su-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        background: var(--ink-faint);
        flex: none;
    }
    .su-led .su-val {
        margin-left: auto;
        color: var(--ink-faint);
        font-weight: 500;
    }
    .su-led.su-on .su-dot { background: var(--go); box-shadow: 0 0 8px var(--go); }
    .su-led.su-warn .su-dot { background: var(--hold); box-shadow: 0 0 8px var(--hold); }
    .su-led.su-bad .su-dot { background: var(--no); box-shadow: 0 0 8px var(--no); }

    /* EBS state strip */
    .su-ebs {
        display: flex;
        flex-direction: column;
        padding: 12px 16px 8px;
        border-radius: 11px;
        border: 1px solid var(--line);
        background: var(--panel);
        font-size: 13px;
    }
    .su-ebs-head {
        display: flex;
        align-items: center;
        gap: 12px;
    }
    .su-ebs-k {
        font-family: var(--mono);
        font-size: 10px;
        letter-spacing: 0.14em;
        text-transform: uppercase;
        color: var(--ink-faint);
    }
    .su-ebs-v { font-weight: 700; }
    .su-ebs-note { margin-left: auto; font-size: 11px; color: var(--ink-faint); }
    .su-ebs-run { border-color: var(--hold-line); }
    .su-ebs-run .su-ebs-v { color: var(--hold-ink); }
    .su-ebs-done { border-color: var(--go-line); }
    .su-ebs-done .su-ebs-v { color: var(--go-ink); }
    .su-ebs-fail { border-color: var(--no-line); background: var(--no-bg); }
    .su-ebs-fail .su-ebs-v { color: var(--no-ink); }

    /* EBS sub-state rail */
    .su-rail {
        display: flex;
        align-items: center;
        margin-top: 12px;
    }
    .su-rnode {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 5px;
        flex: 0 0 auto;
    }
    .su-rdot {
        width: 12px;
        height: 12px;
        border-radius: 50%;
        background: #0c111a;
        border: 1.5px solid var(--line-2);
        transition: 0.2s;
    }
    .su-rlbl {
        font-family: var(--mono);
        font-size: 8.5px;
        font-weight: 600;
        letter-spacing: 0.05em;
        color: var(--ink-faint);
    }
    .su-rline {
        flex: 1;
        height: 2px;
        background: var(--line);
        margin: 0 4px 16px;
        border-radius: 2px;
        transition: 0.2s;
    }
    .su-rline.su-r-done { background: var(--go-line); }
    .su-rline.su-r-err { background: var(--no-line); }
    .su-rnode.su-r-done .su-rdot { background: var(--go); border-color: var(--go); box-shadow: 0 0 7px var(--go); }
    .su-rnode.su-r-done .su-rlbl { color: var(--ink-dim); }
    .su-rnode.su-r-active .su-rdot {
        background: var(--hold);
        border-color: var(--hold);
        box-shadow: 0 0 10px var(--hold);
        animation: su-rpulse 1s ease-in-out infinite;
    }
    .su-rnode.su-r-active .su-rlbl { color: var(--hold-ink); }
    .su-rnode.su-r-err .su-rdot { background: var(--no); border-color: var(--no); box-shadow: 0 0 10px var(--no); }
    .su-rnode.su-r-err .su-rlbl { color: var(--no-ink); }
    @keyframes su-rpulse {
        0%, 100% { box-shadow: 0 0 7px var(--hold); }
        50% { box-shadow: 0 0 14px var(--hold); }
    }
    .su-ebs-coarse {
        margin-top: 6px;
        font-size: 10.5px;
        color: var(--ink-faint);
    }
    .su-ebs-coarse code {
        font-family: var(--mono);
        color: var(--ink-dim);
    }
    .su-diagram {
        margin-top: 12px;
        padding: 8px;
        border-radius: 10px;
        background: #080c12;
        border: 1px solid var(--line);
        background-image:
            linear-gradient(#ffffff04 1px, transparent 1px),
            linear-gradient(90deg, #ffffff04 1px, transparent 1px);
        background-size: 26px 26px;
    }

    /* checklist */
    .su-steps {
        list-style: none;
        margin: 0;
        padding: 6px 18px;
        border: 1px solid var(--line);
        border-radius: 14px;
        background: var(--panel);
    }
    .su-step {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 8px 2px;
        position: relative;
    }
    .su-step:not(:last-child)::before {
        content: '';
        position: absolute;
        left: 11px;
        top: 30px;
        width: 1.5px;
        height: 15px;
        background: var(--line);
    }
    .su-box {
        width: 23px;
        height: 23px;
        border-radius: 50%;
        border: 1.5px solid var(--line-2);
        flex: none;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 11px;
        font-weight: 700;
        color: var(--ink-faint);
        background: var(--panel);
        z-index: 1;
    }
    .su-lbl { font-size: 13px; color: var(--ink-dim); }
    .su-detail {
        margin-left: auto;
        font-family: var(--mono);
        font-size: 11px;
        color: var(--ink-faint);
    }
    .su-step.su-done .su-box { background: var(--go); border-color: var(--go); color: #05230f; }
    .su-step.su-done .su-lbl { color: var(--ink); }
    .su-step.su-done:not(:last-child)::before { background: var(--go-line); }
    .su-step.su-current .su-box {
        border-color: var(--accent);
        color: var(--accent);
        box-shadow: 0 0 0 4px rgba(77, 157, 255, 0.2);
    }
    .su-step.su-current .su-lbl { color: var(--ink); font-weight: 600; }
    .su-step.su-current .su-detail { color: var(--hold-ink); }
    .su-step.su-err .su-box { border-color: var(--no); background: var(--no); color: #2a0508; }
    .su-step.su-err .su-lbl { color: var(--no-ink); }

    /* hint */
    .su-hint {
        padding: 12px 16px;
        border-radius: 12px;
        font-size: 13px;
        border: 1px solid var(--accent-dim);
        background: #142033;
        color: var(--ink);
    }
    .su-hint b { color: var(--accent); margin-right: 4px; }
    .su-hint-success {
        border-color: var(--go-line);
        background: var(--go-bg);
    }
    .su-hint-success b { color: var(--go-ink); }
    .su-hint-alert {
        border-color: var(--no-line);
        background: var(--no-bg);
    }
    .su-hint-alert b { color: var(--no-ink); }

    @media (max-width: 620px) {
        .su-cluster { grid-template-columns: auto 1fr; }
        .su-power { grid-column: 1 / -1; grid-template-columns: repeat(4, 1fr); }
    }
</style>
