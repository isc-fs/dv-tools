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
        indexByTopic,
        overallStatus,
        parseDebug,
    } from './lib/model';

    import AppBar from './lib/components/AppBar.svelte';
    import UpdateBanner from './lib/components/UpdateBanner.svelte';
    import TabBar, { type TabId } from './lib/components/TabBar.svelte';
    import StatusBanner from './lib/components/StatusBanner.svelte';
    import StateHero from './lib/components/StateHero.svelte';
    import FactCards from './lib/components/FactCards.svelte';
    import ResBar from './lib/components/ResBar.svelte';
    import Checklist from './lib/components/Checklist.svelte';
    import RawTopics from './lib/components/RawTopics.svelte';
    import EchoViewer from './lib/components/EchoViewer.svelte';
    import PipelineRoster from './lib/components/PipelineRoster.svelte';
    import KillView from './lib/components/KillView.svelte';

    const POLL_MS = 250;

    // ---- Reactive board state ----
    let topics = $state<TopicSnapshot[]>([]);
    let meta = $state<Meta>({});
    let live = $state<boolean>(false);
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

    // AS state word for the RES/kill fullscreen — pulled from "(AS_XXX)".
    const asWord = $derived.by<string | null>(() => {
        const r = byTopic['/assi/state'];
        if (!r || r.state !== 'ok' || !r.value) return null;
        const m = r.value.match(/\(([^)]+)\)/);
        return m ? m[1] : null;
    });

    const checklistWaiting = $derived(signals.length === 0);

    // Keep the full-viewport red ambient wash in sync with FAULT.
    $effect(() => {
        document.body.classList.toggle('fault', effState === 'fault');
    });

    // ---- Poll loop ----
    async function poll(): Promise<void> {
        try {
            const [data, m] = await Promise.all([getState(), getMeta()]);
            if (m && Object.keys(m).length > 0) meta = m;
            topics = data.topics ?? [];
            // A vanished bound interface (link_lost) is a dead link even though
            // the client object still says "connected" — treat it as down.
            const up = isTauri() ? meta.connected !== false && !meta.link_lost : true;
            live = up;
            liveText = isTauri()
                ? meta.link_lost
                    ? 'link lost'
                    : up
                      ? 'connected'
                      : 'offline'
                : 'demo · live';
        } catch {
            live = false;
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

<AppBar {meta} {live} {liveText} connect={reconnect} armed={standsArmed} />

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

<main>
    <TabBar active={tab} onSelect={(t) => (tab = t)} />

    {#if tab === 'board'}
        <StatusBanner state={effState} tag={overallTag} />

        <PipelineRoster />

        <StateHero
            state={effVerdict.state}
            asRow={byTopic['/assi/state']}
            reason={effVerdict.reason}
        />

        <FactCards {byTopic} />

        <ResBar {byTopic} />

        <section class="lists">
            <Checklist
                title="Safety chain / AS arming"
                names={safetyNames}
                map={signalMap}
                waiting={checklistWaiting}
            />
            <Checklist
                title="Drive readiness"
                names={DRIVE_ORDER}
                map={signalMap}
                waiting={checklistWaiting}
            />
        </section>

        <RawTopics rows={topics} {meta} />
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
