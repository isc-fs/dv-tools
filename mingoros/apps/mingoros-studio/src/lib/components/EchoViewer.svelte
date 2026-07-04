<!--
    Generic topic-echo view. Subscribe to ANY topic on the graph (not just
    the DV-contract set): pick one from the discovered list or type a path,
    then Echo. Standard ROS types decode to readable fields; unknown types
    still show liveness (arrival + rate). Backed by echo_start/echo_tail/
    echo_stop, which pump `subscribe_raw` into a ring buffer on the backend.
-->
<script lang="ts">
    import { onMount } from 'svelte';
    import type { EchoSample, TopicInfo } from '../types';
    import { echoStart, echoStop, echoTail, listTopics } from '../api';

    interface Props {
        live: boolean;
    }
    const { live }: Props = $props();

    const POLL_MS = 250;
    const LIMIT = 300;

    let topics = $state<TopicInfo[]>([]);
    let selected = $state<string>('');
    let custom = $state<string>('');
    let running = $state<boolean>(false);
    let streaming = $state<boolean>(false);
    let samples = $state<EchoSample[]>([]);
    let total = $state<number>(0);
    let err = $state<string>('');
    let typeName = $state<string>('');
    let autoscroll = $state<boolean>(true);
    let listEl = $state<HTMLDivElement | null>(null);

    // The chosen topic: a free-typed path takes precedence over the dropdown.
    const topic = $derived((custom.trim() || selected).trim());

    // Publish rate over the visible sample window.
    const rate = $derived.by<number | null>(() => {
        if (samples.length < 2) return null;
        const dt = (samples[samples.length - 1].t_ms - samples[0].t_ms) / 1000;
        return dt > 0 ? (samples.length - 1) / dt : null;
    });

    onMount(() => {
        void refreshTopics();
        const id = setInterval(() => {
            if (running) void pump();
        }, POLL_MS);
        return () => {
            clearInterval(id);
            if (running) void echoStop();
        };
    });

    // Keep the stream pinned to the newest row when following.
    $effect(() => {
        samples.length;
        if (autoscroll && listEl) listEl.scrollTop = listEl.scrollHeight;
    });

    async function refreshTopics(): Promise<void> {
        try {
            topics = await listTopics();
        } catch {
            topics = []; // not connected yet — free-typing still works
        }
    }

    async function start(): Promise<void> {
        if (!topic) return;
        err = '';
        samples = [];
        total = 0;
        try {
            await echoStart(topic);
            running = true;
            streaming = true;
            typeName = topics.find((t) => t.name === topic)?.type_name ?? '';
        } catch (e) {
            err = e instanceof Error ? e.message : String(e);
            running = false;
        }
    }

    async function stop(): Promise<void> {
        running = false;
        try {
            await echoStop();
        } catch {
            /* best-effort */
        }
    }

    async function pump(): Promise<void> {
        try {
            const tail = await echoTail(LIMIT);
            samples = tail.samples;
            total = tail.total;
            streaming = tail.running;
        } catch (e) {
            err = e instanceof Error ? e.message : String(e);
        }
    }
</script>

<section class="echo">
    <div class="echo-controls">
        <select class="echo-select" bind:value={selected} disabled={running}>
            <option value="">— pick a topic —</option>
            {#each topics as t (t.name)}
                <option value={t.name}>{t.name} · {t.type_name}</option>
            {/each}
        </select>
        <input
            class="echo-custom"
            placeholder="…or type any /topic"
            bind:value={custom}
            disabled={running}
            spellcheck="false"
        />
        {#if running}
            <button type="button" class="echo-stop" onclick={() => void stop()}
                >Stop</button
            >
        {:else}
            <button
                type="button"
                class="echo-go"
                onclick={() => void start()}
                disabled={!topic}>Echo</button
            >
        {/if}
        <button
            type="button"
            class="echo-refresh"
            title="Refresh topic list"
            onclick={() => void refreshTopics()}
            disabled={running}>↻</button
        >
    </div>

    {#if err}<div class="echo-err">{err}</div>{/if}

    {#if running}
        <div class="echo-metabar">
            <span class="echo-topic">{topic}</span>
            {#if typeName}<span class="echo-type">{typeName}</span>{/if}
            <span class="grow"></span>
            <span class="echo-stat">{total} msgs</span>
            {#if rate != null}<span class="echo-stat">{rate.toFixed(1)} Hz</span>{/if}
            <span class="echo-live {streaming ? 'on' : 'off'}"
                >{streaming ? '● live' : '■ ended'}</span
            >
            <label class="echo-follow"
                ><input type="checkbox" bind:checked={autoscroll} /> follow</label
            >
        </div>
        <div class="echo-stream" bind:this={listEl}>
            {#each samples as s (s.seq)}
                <div class="echo-row">
                    <span class="echo-seq">{s.seq}</span>
                    <span class="echo-tt">t+{(s.t_ms / 1000).toFixed(2)}s</span>
                    <span class="echo-val">{s.summary}</span>
                </div>
            {:else}
                <div class="echo-empty">
                    waiting for the first message… (a silent topic ends after ~20 s)
                </div>
            {/each}
        </div>
    {:else}
        <div class="echo-idle">
            <p>
                Echo <strong>any</strong> topic on the graph — pick one above or type
                a path, then <em>Echo</em>.
            </p>
            <p class="echo-note">
                Standard ROS types (std_msgs, geometry_msgs, nav_msgs, sensor_msgs)
                decode to readable fields; other types still show liveness — arrival
                + rate + type.{#if !live}<br />Demo mode — synthetic data.{/if}
            </p>
        </div>
    {/if}
</section>
