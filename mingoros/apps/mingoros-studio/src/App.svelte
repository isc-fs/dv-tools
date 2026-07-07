<!--
    ISC MingoROS — Go / No-Go board root.

    Owns the reactive board state, runs the 250 ms poll loop against
    the Tauri backend (or the baked demo when standalone), and composes
    the child components. All parsing / classification / verdict logic
    lives in lib/model.ts; this file only wires data to views and keeps
    the body.fault ambient wash in sync.
-->
<script lang="ts">
    import { onMount } from 'svelte';

    import type {
        Meta,
        OverallState,
        ParsedSignal,
        TopicSnapshot,
    } from './lib/types';
    import { connect, getMeta, getState, isTauri } from './lib/api';
    import {
        DRIVE_ORDER,
        SAFETY_ORDER,
        aggregate,
        deriveVerdict,
        extractWord,
        fmtAge,
        indexByTopic,
        overallStatus,
        parseDebug,
    } from './lib/model';

    import AppBar from './lib/components/AppBar.svelte';
    import UpdateBanner from './lib/components/UpdateBanner.svelte';
    import TabBar, { type TabId } from './lib/components/TabBar.svelte';
    import StateHero from './lib/components/StateHero.svelte';
    import FactCards from './lib/components/FactCards.svelte';
    import ResBar from './lib/components/ResBar.svelte';
    import Checklist from './lib/components/Checklist.svelte';
    import EchoViewer from './lib/components/EchoViewer.svelte';
    import PipelineRoster from './lib/components/PipelineRoster.svelte';
    import KillView from './lib/components/KillView.svelte';
    import SessionRecorder from './lib/components/SessionRecorder.svelte';
    import Details from './lib/components/Details.svelte';
    import StartupGuide from './lib/components/StartupGuide.svelte';
    import { recorder } from './lib/recorderStore.svelte';

    const POLL_MS = 250;

    // ---- Reactive board state ----
    let topics = $state<TopicSnapshot[]>([]);
    let meta = $state<Meta>({});
    let live = $state<boolean>(false);
    // DDS is up but no live data is arriving (the app is only seeing its own
    // node) — a distinct "no data" state so CONNECTED never lies about the car.
    let linkWarn = $state<boolean>(false);
    let liveText = $state<string>('connecting');
    let tab = $state<TabId>('board');
    let killView = $state<boolean>(false);
    // Stands interlock (#60): actuation (EBS + future self-tests) is LOCKED
    // until the operator explicitly confirms the car is on stands. Default off.
    let standsArmed = $state<boolean>(false);

    // ---- Derivations (all pure, from lib/model) ----
    const byTopic = $derived(indexByTopic(topics));

    const debugValue = $derived.by<string | null>(() => {
        const r = byTopic['/debug'];
        return r && r.state === 'ok' ? r.value : null;
    });
    const signals = $derived<ParsedSignal[]>(parseDebug(debugValue));

    // lower-cased name -> signal, shared by both checklists.
    const signalMap = $derived.by<Record<string, ParsedSignal>>(() => {
        const m: Record<string, ParsedSignal> = {};
        for (const s of signals) m[s.name.toLowerCase()] = s;
        return m;
    });

    // Any unknown token (not in either order list, not RES) is appended
    // to the safety group so it is never silently dropped.
    const safetyNames = $derived.by<string[]>(() => {
        const known = new Set(
            [...SAFETY_ORDER, ...DRIVE_ORDER, 'RES'].map((x) => x.toLowerCase()),
        );
        const extras = signals
            .filter((s) => !known.has(s.name.toLowerCase()))
            .map((s) => s.name);
        return [...SAFETY_ORDER, ...extras];
    });

    const agg = $derived(aggregate(topics));

    const overallState = $derived<OverallState>(
        overallStatus(topics, signals),
    );

    const overallTag = $derived(agg.okCount + '/' + topics.length + ' topics live');

    // Overall-state word + one-liner for the verdict-band header (folds in the
    // old StatusBanner, which "reads first" ahead of the rotated stamp).
    const OVERALL_COPY: Record<OverallState, { h: string; d: string }> = {
        fault: { h: 'FAULT', d: 'Active safety fault — the car must not move.' },
        hold: { h: 'STALE HEARTBEAT', d: 'A live topic went silent past the watchdog.' },
        go: { h: 'NOMINAL', d: 'All monitored safety signals healthy and fresh.' },
        standby: { h: 'WAITING FOR DATA…', d: 'No topics received yet.' },
    };

    const verdict = $derived(
        deriveVerdict(topics, signals, byTopic, meta.watchdog_s ?? 1.5),
    );

    // A vanished bound interface (link_lost, #86) is a hard fault for the whole
    // verdict — not just the LED: the readings are no longer live, so the board
    // must NOT be trusted as READY. Overrides the derived state everywhere (#56).
    const linkLost = $derived(isTauri() && meta.link_lost === true);
    const effState = $derived<OverallState>(linkLost ? 'fault' : overallState);
    const effVerdict = $derived<{ state: OverallState; reason: string }>(
        linkLost
            ? { state: 'fault', reason: 'LINK LOST — the bound interface is gone; readings are not live' }
            : verdict,
    );
    const overallCopy = $derived(OVERALL_COPY[effState]);

    // uDV → micro_ros_agent → DDS link-health verdict (#61): the uDV's heartbeat
    // topics (/debug, /assi/state) arriving fresh means the whole chain is
    // alive (uDV powered → agent bridging → DDS delivering). Stale = a hiccup;
    // absent = the chain is down (agent off / uDV off / wrong domain).
    const udvLink = $derived.by<{ state: 'ok' | 'stale' | 'down'; detail: string }>(() => {
        const beat = byTopic['/debug'] ?? byTopic['/assi/state'];
        if (!beat || beat.state !== 'ok') {
            return { state: 'down', detail: 'no uDV heartbeat — agent off / uDV off / wrong domain' };
        }
        if (!beat.fresh) {
            return { state: 'stale', detail: `heartbeat stale (${fmtAge(beat.age_ms)}) — agent/uDV hiccup` };
        }
        return { state: 'ok', detail: `heartbeat live · ${fmtAge(beat.age_ms)}` };
    });

    // AS state word for the RES/kill fullscreen — pulled from "(AS_XXX)".
    const asWord = $derived.by<string | null>(() => {
        const r = byTopic['/assi/state'];
        if (!r || r.state !== 'ok' || !r.value) return null;
        const m = r.value.match(/\(([^)]+)\)/);
        return m ? m[1] : null;
    });

    const checklistWaiting = $derived(signals.length === 0);

    // Are we actually receiving live data from the car? A discovered topic can
    // just be the app's own subscription (no publisher), so "DDS is up" ≠
    // "connected". The honest signal is a FRESH priority sample arriving.
    const freshTopics = $derived(topics.filter((t) => t.state === 'ok' && t.fresh));
    const receiving = $derived(freshTopics.length > 0);

    // Startup-tab inputs: the selected mission word (/ami/mission) and whether
    // the DV pipeline is up (/dv/status fresh) — read-only, for the live guide.
    const missionWord = $derived.by<string | null>(() => {
        const r = byTopic['/ami/mission'];
        return r && r.state === 'ok' ? extractWord(r.value) : null;
    });
    const dvpcUp = $derived.by<boolean>(() => {
        const r = byTopic['/dv/status'];
        return !!r && r.state === 'ok' && r.fresh;
    });

    // Keep the full-viewport red ambient wash in sync with FAULT.
    $effect(() => {
        document.body.classList.toggle('fault', effState === 'fault');
    });

    // Feed the session recorder every poll so it captures AS/verdict
    // transitions no matter which tab is showing (the record toggle lives on
    // the board, the debrief on Details — capture must never pause).
    $effect(() => {
        recorder.observe(asWord, effVerdict.state);
    });

    // ---- Poll loop ----
    async function poll(): Promise<void> {
        try {
            const [data, m] = await Promise.all([getState(), getMeta()]);
            if (m && Object.keys(m).length > 0) meta = m;
            topics = data.topics ?? [];
            // "Connected" must mean the CAR is delivering data, not just that
            // the DDS participant came up. `up` = DDS reachable; `recv` = a
            // fresh priority sample is actually arriving.
            const up = isTauri() ? meta.connected !== false && !meta.link_lost : true;
            const recv = (data.topics ?? []).some((t) => t.state === 'ok' && t.fresh);
            live = isTauri() ? up && recv : true;
            linkWarn = isTauri() && up && !recv && meta.link_lost !== true;
            liveText = !isTauri()
                ? 'demo · live'
                : meta.link_lost
                  ? 'link lost'
                  : !up
                    ? 'offline'
                    : recv
                      ? 'connected'
                      : 'no data';
        } catch {
            live = false;
            linkWarn = false;
            liveText = 'disconnected';
        }
    }

    async function reconnect(domain: number, iface: string): Promise<void> {
        await connect(domain, iface);
        meta = await getMeta();
    }

    onMount(() => {
        meta = { domain: 0 };
        void (async () => {
            meta = await getMeta();
            await poll();
        })();
        const id = setInterval(() => void poll(), POLL_MS);
        return () => {
            clearInterval(id);
            document.body.classList.remove('fault');
        };
    });
</script>

<div id="ambient" aria-hidden="true"></div>

<!-- The connection bar + update banner + safety strip stick to the top as ONE
     unit, so the (safety-critical) strip never slides over the wrapping backend
     row of the bar. -->
<div class="app-header">
<AppBar
    {meta}
    {live}
    {linkWarn}
    {liveText}
    connect={reconnect}
    armed={standsArmed}
    liveCount={freshTopics.length}
    topicTotal={topics.length}
/>

<UpdateBanner />

<div class="safety-strip" role="alert">
    <span class="warn-glyph" aria-hidden="true">▲</span>
    <span
        ><b>CAR ON STANDS · WHEELS OFF THE GROUND, ALWAYS.</b> ISC MingoROS can command
        actuation (EBS, control/mission topics) — a stray command can move the car.
        Never use it with the wheels able to touch down.</span
    >
    <span class="grow"></span>
    <button
        type="button"
        class="stands-interlock"
        class:armed={standsArmed}
        aria-pressed={standsArmed}
        onclick={() => (standsArmed = !standsArmed)}
        title={standsArmed
            ? 'Actuation ARMED — click to lock. Only while the car is genuinely on stands.'
            : 'Actuation LOCKED. Click to arm — confirms the car is on stands, wheels off the ground.'}
    >
        {standsArmed ? '🔓 ON STANDS · actuation armed' : '🔒 actuation locked — arm (on stands)'}
    </button>
</div>
</div>

<main>
    <TabBar active={tab} onSelect={(t) => (tab = t)} />

    {#if tab === 'board'}
        <!-- Single-viewport board: a full-width verdict band (TIER 1) over a
             3-column gauge deck (TIER 2) whose height is its tallest column. -->
        <div class="board">
            <div class="verdict-band">
                <div class="band-head band-{effState}">
                    <span class="beacon"></span>
                    <span class="bh-word">{overallCopy.h}</span>
                    <span class="bh-desc">{overallCopy.d}</span>
                    <span class="otag">{overallTag}</span>
                    <div class="udv-link udv-{udvLink.state}" title={udvLink.detail}>
                        <span class="udv-dot"></span>
                        <span class="udv-label">uDV LINK</span>
                        <span class="udv-state"
                            >{udvLink.state === 'ok'
                                ? 'live'
                                : udvLink.state === 'stale'
                                  ? 'STALE'
                                  : 'DOWN'}</span
                        >
                        <span class="udv-detail">{udvLink.detail}</span>
                    </div>
                </div>

                <StateHero
                    state={effVerdict.state}
                    asRow={byTopic['/assi/state']}
                    reason={effVerdict.reason}
                />
            </div>

            <div class="deck">
                <div class="safety">
                    <Checklist
                        title="Safety chain / AS arming"
                        names={safetyNames}
                        map={signalMap}
                        waiting={checklistWaiting}
                    />
                </div>
                <div class="drive">
                    <Checklist
                        title="Drive readiness"
                        names={DRIVE_ORDER}
                        map={signalMap}
                        waiting={checklistWaiting}
                    />
                </div>
                <div class="gauges">
                    <ResBar {byTopic} />
                    <FactCards {byTopic} />
                    <PipelineRoster {receiving} />
                    <SessionRecorder />
                </div>
            </div>
        </div>
    {:else if tab === 'startup'}
        <StartupGuide
            {signalMap}
            {asWord}
            {receiving}
            {missionWord}
            {dvpcUp}
        />
    {:else if tab === 'details'}
        <Details rows={topics} {meta} />
    {:else}
        <EchoViewer live={isTauri()} watchdogS={meta.watchdog_s ?? 1.5} />
    {/if}
</main>

<button
    type="button"
    class="resview-btn"
    title="Fullscreen RES / kill-decision view — glanceable safety verdict"
    onclick={() => (killView = true)}>RES VIEW</button
>

{#if killView}
    <KillView
        state={effVerdict.state}
        reason={effVerdict.reason}
        {asWord}
        linkLost={meta.link_lost ?? false}
        onClose={() => (killView = false)}
    />
{/if}
