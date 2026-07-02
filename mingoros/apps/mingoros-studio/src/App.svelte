<!--
    MingoROS Studio — Go / No-Go board root.

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
    import StatusBanner from './lib/components/StatusBanner.svelte';
    import StateHero from './lib/components/StateHero.svelte';
    import FactCards from './lib/components/FactCards.svelte';
    import ResBar from './lib/components/ResBar.svelte';
    import Checklist from './lib/components/Checklist.svelte';
    import RawTopics from './lib/components/RawTopics.svelte';

    const POLL_MS = 250;

    // ---- Reactive board state ----
    let topics = $state<TopicSnapshot[]>([]);
    let meta = $state<Meta>({});
    let live = $state<boolean>(false);
    let liveText = $state<string>('connecting');

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

    const checklistWaiting = $derived(signals.length === 0);

    // Keep the full-viewport red ambient wash in sync with FAULT.
    $effect(() => {
        document.body.classList.toggle('fault', overallState === 'fault');
    });

    // ---- Poll loop ----
    async function poll(): Promise<void> {
        try {
            const [data, m] = await Promise.all([getState(), getMeta()]);
            if (m && Object.keys(m).length > 0) meta = m;
            topics = data.topics ?? [];
            const up = isTauri() ? meta.connected !== false : true;
            live = up;
            liveText = isTauri()
                ? up
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

<AppBar {meta} {live} {liveText} connect={reconnect} />

<main>
    <StatusBanner state={overallState} tag={overallTag} />

    <StateHero
        state={verdict.state}
        asRow={byTopic['/assi/state']}
        reason={verdict.reason}
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
</main>
