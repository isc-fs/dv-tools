<!--
    Generic topic-echo view. Add ANY number of topics on the graph (not just
    the DV-contract set) — pick from the discovered list or type a path. Their
    messages interleave in one colour-coded stream, tagged by topic. Standard
    ROS types decode to readable fields; unknown types still show liveness
    (arrival + rate). Backed by echo_add / echo_remove / echo_clear / echo_tail,
    which pump `subscribe_raw` into a shared ring buffer on the backend.
-->
<script lang="ts">
    import { onMount } from 'svelte';
    import type { EchoSample, EchoTopicStatus, TopicInfo } from '../types';
    import { echoAdd, echoClear, echoRemove, echoTail, listTopics } from '../api';

    interface Props {
        live: boolean;
    }
    const { live }: Props = $props();

    const POLL_MS = 250;
    const LIMIT = 400;
    // Distinct hues that read on the dark ground; assigned by add-order.
    const PALETTE = [
        '#4d9dff', '#35d07f', '#f0b429', '#ff6b6b',
        '#b388ff', '#4dd0e1', '#ff9e64', '#9ccc65',
    ];

    let topics = $state<TopicInfo[]>([]);
    let selected = $state<string>('');
    let custom = $state<string>('');
    let active = $state<EchoTopicStatus[]>([]);
    let samples = $state<EchoSample[]>([]);
    let err = $state<string>('');
    let autoscroll = $state<boolean>(true);
    let listEl = $state<HTMLDivElement | null>(null);

    // The topic to add: a free-typed path wins over the dropdown.
    const topic = $derived((custom.trim() || selected).trim());

    // Only offer topics not already active in the picker.
    const pickable = $derived(
        topics.filter((t) => !active.some((a) => a.topic === t.name)),
    );

    // Stable per-topic colour, keyed by position in the active list.
    const colorMap = $derived.by(() => {
        const m = new Map<string, string>();
        active.forEach((t, i) => m.set(t.topic, PALETTE[i % PALETTE.length]));
        return m;
    });
    const colorOf = (t: string): string => colorMap.get(t) ?? '#8b98ab';

    // Per-topic count + rate over the visible window.
    const statMap = $derived.by(() => {
        const g = new Map<string, EchoSample[]>();
        for (const s of samples) {
            const arr = g.get(s.topic);
            if (arr) arr.push(s);
            else g.set(s.topic, [s]);
        }
        const m = new Map<string, string>();
        for (const [t, arr] of g) {
            let label = `${arr.length}`;
            if (arr.length >= 2) {
                const dt = (arr[arr.length - 1].t_ms - arr[0].t_ms) / 1000;
                if (dt > 0) label += ` · ${((arr.length - 1) / dt).toFixed(1)} Hz`;
            }
            m.set(t, label);
        }
        return m;
    });
    const statOf = (t: string): string => statMap.get(t) ?? '0';

    onMount(() => {
        void refreshTopics();
        void pump();
        const id = setInterval(() => void pump(), POLL_MS);
        // Deliberately do NOT clear the backend session on unmount — switching
        // to the board tab and back keeps the echo running.
        return () => clearInterval(id);
    });

    $effect(() => {
        samples.length;
        if (autoscroll && listEl) listEl.scrollTop = listEl.scrollHeight;
    });

    async function refreshTopics(): Promise<void> {
        try {
            topics = await listTopics();
        } catch {
            topics = [];
        }
    }

    async function add(): Promise<void> {
        if (!topic) return;
        err = '';
        const t = topic;
        try {
            await echoAdd(t);
            selected = '';
            custom = '';
            await pump();
        } catch (e) {
            err = e instanceof Error ? e.message : String(e);
        }
    }

    async function remove(t: string): Promise<void> {
        try {
            await echoRemove(t);
            await pump();
        } catch {
            /* best-effort */
        }
    }

    async function clearAll(): Promise<void> {
        try {
            await echoClear();
            samples = [];
            active = [];
        } catch {
            /* best-effort */
        }
    }

    async function pump(): Promise<void> {
        try {
            const tail = await echoTail(LIMIT);
            active = tail.topics;
            samples = tail.samples;
        } catch (e) {
            err = e instanceof Error ? e.message : String(e);
        }
    }
</script>

<section class="echo">
    <div class="echo-controls">
        <select class="echo-select" bind:value={selected}>
            <option value="">— add a topic —</option>
            {#each pickable as t (t.name)}
                <option value={t.name}>{t.name} · {t.type_name}</option>
            {/each}
        </select>
        <input
            class="echo-custom"
            placeholder="…or type any /topic"
            bind:value={custom}
            spellcheck="false"
            onkeydown={(e) => {
                if (e.key === 'Enter') void add();
            }}
        />
        <button type="button" class="echo-go" onclick={() => void add()} disabled={!topic}
            >Add</button
        >
        <button
            type="button"
            class="echo-refresh"
            title="Refresh topic list"
            onclick={() => void refreshTopics()}>↻</button
        >
        {#if active.length}
            <button type="button" class="echo-clear-btn" onclick={() => void clearAll()}
                >Clear all</button
            >
        {/if}
    </div>

    {#if err}<div class="echo-err">{err}</div>{/if}

    {#if active.length}
        <div class="echo-chips">
            {#each active as t (t.topic)}
                <span class="echo-chip" style="--c:{colorOf(t.topic)}">
                    <span class="echo-chip-dot"></span>
                    <span class="echo-chip-name">{t.topic}</span>
                    <span class="echo-chip-stat">{statOf(t.topic)}</span>
                    {#if !t.running}<span class="echo-chip-ended">ended</span>{/if}
                    <button
                        type="button"
                        class="echo-chip-x"
                        title="Remove {t.topic}"
                        onclick={() => void remove(t.topic)}>×</button
                    >
                </span>
            {/each}
            <span class="grow"></span>
            <label class="echo-follow"
                ><input type="checkbox" bind:checked={autoscroll} /> follow</label
            >
        </div>
        <div class="echo-stream" bind:this={listEl}>
            {#each samples as s (s.topic + ':' + s.seq)}
                <div class="echo-row">
                    <span class="echo-rowtopic" style="color:{colorOf(s.topic)}"
                        >{s.topic}</span
                    >
                    <span class="echo-tt">t+{(s.t_ms / 1000).toFixed(2)}s</span>
                    <span class="echo-val">{s.summary}</span>
                </div>
            {:else}
                <div class="echo-empty">
                    waiting for messages… (a silent topic ends after ~20 s)
                </div>
            {/each}
        </div>
    {:else}
        <div class="echo-idle">
            <p>
                Echo <strong>any</strong> topics on the graph — add one or more from
                the list or by path. They stream together, colour-coded per topic.
            </p>
            <p class="echo-note">
                Standard ROS types (std_msgs, geometry_msgs, nav_msgs, sensor_msgs)
                decode to readable fields; other types still show liveness — arrival
                + rate + type.{#if !live}<br />Demo mode — synthetic data.{/if}
            </p>
        </div>
    {/if}
</section>
