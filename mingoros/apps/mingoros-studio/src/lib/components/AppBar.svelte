<!--
    Connection bar — brand, ROS-domain + local-interface inputs, Connect
    button, backend label (with discovered-topic count), and the live LED.
    When standalone (no Tauri backend) the inputs are disabled and the LED
    shows "demo · live". A connect error surfaces as a chip in the bar.
-->
<script lang="ts">
    import { onMount } from 'svelte';
    import type { Meta, NetInterface } from '../types';
    import { isTauri, listInterfaces } from '../api';
    import EbsControl from './EbsControl.svelte';

    interface Props {
        meta: Meta;
        /** True when the backend link is up (drives the LED). */
        live: boolean;
        /** Text shown next to the LED ("connected" / "offline" / …). */
        liveText: string;
        /** Reconnect callback — validated domain id + optional interface IP. */
        connect: (domain: number, iface: string) => Promise<void>;
    }

    const { meta, live, liveText, connect }: Props = $props();

    const tauri = isTauri();

    // Local input state seeded from meta; kept in sync when the backend
    // reports new values and the user isn't mid-edit.
    let domainStr = $state<string>('0');
    let ifaceStr = $state<string>('');
    let invalid = $state<boolean>(false);
    let busy = $state<boolean>(false);
    let touched = $state<boolean>(false);
    let interfaces = $state<NetInterface[]>([]);

    async function refreshIfaces(): Promise<void> {
        try {
            interfaces = await listInterfaces();
        } catch {
            interfaces = [];
        }
    }
    onMount(() => {
        void refreshIfaces();
    });

    $effect(() => {
        // Reseed from the backend only while the field is pristine, so we
        // never clobber what the operator is typing.
        if (!touched && !busy) {
            domainStr = String(meta.domain ?? 0);
            if (meta.iface != null) {
                ifaceStr = meta.iface;
            }
        }
    });

    const backendLabel = $derived.by<string>(() => {
        let s = meta.backend || '—';
        if (meta.domain != null) s += ' · dom ' + meta.domain;
        if (meta.iface) s += ' · ' + meta.iface;
        if (meta.discovered != null) s += ' · ' + meta.discovered + ' topics';
        return s;
    });

    async function submit(): Promise<void> {
        const raw = domainStr.trim();
        const dom = parseInt(raw, 10);
        if (!/^\d+$/.test(raw) || isNaN(dom) || dom < 0) {
            invalid = true;
            return;
        }
        invalid = false;
        domainStr = String(dom);
        busy = true;
        try {
            await connect(dom, ifaceStr.trim());
        } catch {
            /* error surfaces via meta.error on the next poll */
        }
        busy = false;
        touched = false;
    }

    function onKeydown(e: KeyboardEvent): void {
        if (e.key === 'Enter') {
            e.preventDefault();
            void submit();
        }
    }

    function onInput(): void {
        touched = true;
        invalid = false;
    }
</script>

<header class="topbar">
    <div class="brand">
        <span class="mark"><span class="isc">ISC</span> MINGO<em>ROS</em></span>
        <span class="sub">Go / No-Go board</span>
    </div>
    <div class="grow"></div>
    {#if meta.error}
        <div class="conn-err" title={meta.error}>⚠ {meta.error}</div>
    {/if}
    <div class="conn">
        {#if tauri}
            <label for="dom">domain</label>
            <input
                id="dom"
                class="dom-in"
                inputmode="numeric"
                aria-label="ROS domain id"
                aria-invalid={invalid ? 'true' : undefined}
                disabled={busy}
                bind:value={domainStr}
                oninput={onInput}
                onkeydown={onKeydown}
            />
            <label for="iface">iface</label>
            <select
                id="iface"
                class="iface-sel"
                aria-label="local interface to bind DDS to"
                title="Bind DDS to your direct-link Ethernet so discovery goes over the cable (auto = all interfaces)"
                disabled={busy}
                bind:value={ifaceStr}
                onchange={onInput}
                onfocus={() => void refreshIfaces()}
            >
                <option value="">auto (all interfaces)</option>
                {#each interfaces as i (i.name + i.ip)}
                    <option value={i.ip}
                        >{i.name} · {i.ip}{i.loopback ? ' (lo)' : ''}</option
                    >
                {/each}
            </select>
            <button type="button" disabled={busy} onclick={() => void submit()}>
                {busy ? 'connecting…' : 'Connect'}
            </button>
        {:else}
            <label for="dom">domain</label>
            <input
                id="dom"
                class="dom-in"
                value={meta.domain ?? 0}
                disabled
                aria-label="ROS domain id"
            />
            <button
                type="button"
                disabled
                title="Connect is available in the desktop app">Connect</button
            >
        {/if}
    </div>
    <EbsControl />
    <div class="link"><span>backend</span> <b>{backendLabel}</b></div>
    <div class="live" class:on={live} class:off={!live}>
        <span class="led"></span><span>{liveText}</span>
    </div>
</header>
