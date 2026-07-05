<!--
    Decoded-state session recorder + auto debrief card (#55).

    Records the board's decoded state TRANSITIONS (AS state + Go/No-Go verdict)
    over a bench session — no raw bag, just the meaning — and on stop renders a
    debrief: how long, which AS states and for how long, how many faults and
    when. A lightweight "what just happened" card for post-run review without
    replaying anything.
-->
<script lang="ts">
    import type { OverallState } from '../types';

    interface Props {
        asWord: string | null;
        verdictState: OverallState;
    }
    const { asWord, verdictState }: Props = $props();

    interface Ev {
        t: number; // ms since record start
        kind: 'AS' | 'verdict';
        to: string;
    }

    let recording = $state<boolean>(false);
    let startT = 0;
    let now = $state<number>(0);
    let events = $state<Ev[]>([]);
    let lastAs = '';
    let lastVerdict = '';

    // Capture transitions while recording (guarded so it can't loop).
    $effect(() => {
        // track deps
        const as = asWord ?? '—';
        const vs = verdictState;
        if (!recording) return;
        const t = Date.now() - startT;
        now = t;
        if (as !== lastAs) {
            lastAs = as;
            events = [...events, { t, kind: 'AS', to: as }];
        }
        if (vs !== lastVerdict) {
            lastVerdict = vs;
            events = [...events, { t, kind: 'verdict', to: vs }];
        }
    });

    function start(): void {
        events = [];
        startT = Date.now();
        now = 0;
        lastAs = asWord ?? '—';
        lastVerdict = verdictState;
        // seed the initial state as the first entries
        events = [
            { t: 0, kind: 'AS', to: lastAs },
            { t: 0, kind: 'verdict', to: lastVerdict },
        ];
        recording = true;
    }
    function stop(): void {
        recording = false;
    }

    const fmtT = (ms: number): string => `${(ms / 1000).toFixed(1)}s`;

    // Debrief: total duration, time-in-AS-state, fault count + first fault.
    const debrief = $derived.by(() => {
        const dur = recording ? now : events.length ? events[events.length - 1].t : 0;
        // time in each AS state (span until the next AS event or session end)
        const asEvents = events.filter((e) => e.kind === 'AS');
        const dwell = new Map<string, number>();
        for (let i = 0; i < asEvents.length; i++) {
            const end = i + 1 < asEvents.length ? asEvents[i + 1].t : dur;
            dwell.set(asEvents[i].to, (dwell.get(asEvents[i].to) ?? 0) + (end - asEvents[i].t));
        }
        const faults = events.filter((e) => e.kind === 'verdict' && e.to === 'fault');
        return {
            dur,
            transitions: events.length,
            dwell: [...dwell.entries()].sort((a, b) => b[1] - a[1]),
            faultCount: faults.length,
            firstFault: faults.length ? faults[0].t : null,
        };
    });
</script>

<section class="recorder">
    <div class="rec-head">
        <button
            type="button"
            class="rec-btn"
            class:on={recording}
            onclick={() => (recording ? stop() : start())}
        >
            {recording ? '■ Stop' : '⏺ Record session'}
        </button>
        {#if recording}
            <span class="rec-live">● REC {fmtT(now)} · {events.length} transitions</span>
        {/if}
    </div>

    {#if !recording && events.length > 0}
        <div class="debrief">
            <div class="db-title">SESSION DEBRIEF</div>
            <div class="db-row">
                <span class="db-k">duration</span><span class="db-v">{fmtT(debrief.dur)}</span>
                <span class="db-k">transitions</span><span class="db-v">{debrief.transitions}</span>
                <span class="db-k">faults</span>
                <span class="db-v" class:bad={debrief.faultCount > 0}>
                    {debrief.faultCount}{#if debrief.firstFault != null}
                        (first at {fmtT(debrief.firstFault)}){/if}
                </span>
            </div>
            {#if debrief.dwell.length}
                <div class="db-dwell">
                    {#each debrief.dwell as [state, ms] (state)}
                        <span class="db-chip"
                            ><b>{state}</b> {fmtT(ms)}</span
                        >
                    {/each}
                </div>
            {/if}
        </div>
    {/if}
</section>
