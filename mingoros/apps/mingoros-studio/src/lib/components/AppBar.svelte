<!--
    Connection bar — brand, ROS-domain input, Connect button, backend
    label and the live/connected LED. When running standalone (no
    Tauri backend) the domain input + button are disabled and the LED
    shows "demo · live"; the real app wires Connect to the backend.
-->
<script lang="ts">
    import type { Meta } from '../types';
    import { isTauri } from '../api';
    import EbsControl from './EbsControl.svelte';

    interface Props {
        meta: Meta;
        /** True when the backend link is up (drives the LED). */
        live: boolean;
        /** Text shown next to the LED ("connected" / "offline" / …). */
        liveText: string;
        /** Reconnect callback — validated domain id. */
        connect: (domain: number) => Promise<void>;
    }

    const { meta, live, liveText, connect }: Props = $props();

    const tauri = isTauri();

    // Local input state seeded from meta.domain; kept in sync when the
    // backend reports a new domain and the user isn't mid-edit. The
    // seed lands via the $effect below (not the initializer) so it
    // tracks meta reactively rather than capturing its first value.
    let domainStr = $state<string>('0');
    let invalid = $state<boolean>(false);
    let busy = $state<boolean>(false);
    let touched = $state<boolean>(false);

    $effect(() => {
        // Reseed from the backend only while the field is pristine, so
        // we never clobber what the operator is typing.
        if (!touched && !busy) {
            domainStr = String(meta.domain ?? 0);
        }
    });

    const backendLabel = $derived(
        (meta.backend || '—') +
            (meta.domain != null ? ' · dom ' + meta.domain : ''),
    );

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
            await connect(dom);
        } catch {
            /* keep last known meta; error surfaces via meta on next poll */
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
        <span class="mark">MINGO<em>ROS</em></span>
        <span class="sub">Go / No-Go board</span>
    </div>
    <div class="grow"></div>
    <div class="conn">
        {#if tauri}
            <label for="dom">ROS domain</label>
            <input
                id="dom"
                inputmode="numeric"
                aria-label="ROS domain id"
                aria-invalid={invalid ? 'true' : undefined}
                disabled={busy}
                bind:value={domainStr}
                oninput={onInput}
                onkeydown={onKeydown}
            />
            <button type="button" disabled={busy} onclick={() => void submit()}>
                {busy ? 'connecting…' : 'Connect'}
            </button>
        {:else}
            <label for="dom">ROS domain</label>
            <input
                id="dom"
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
