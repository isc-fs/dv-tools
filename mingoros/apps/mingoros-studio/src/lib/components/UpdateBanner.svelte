<!--
    Auto-update banner. Checks for a signed update on mount; if one is
    available a slim bar offers "Install & restart" (with download progress).
    Hidden entirely when up-to-date or running standalone (browser demo).
-->
<script lang="ts">
    import { onMount } from 'svelte';
    import type { Update } from '@tauri-apps/plugin-updater';
    import { checkForUpdate, installUpdate } from '../updater';

    let phase = $state<'idle' | 'available' | 'installing' | 'error'>('idle');
    let version = $state<string>('');
    let pct = $state<number | null>(null);
    let error = $state<string>('');
    let dismissed = $state<boolean>(false);
    let handle: Update | null = null;

    onMount(() => {
        void (async () => {
            const res = await checkForUpdate();
            if (res) {
                handle = res.update;
                version = res.version;
                phase = 'available';
            }
        })();
    });

    async function install(): Promise<void> {
        if (!handle) return;
        phase = 'installing';
        error = '';
        pct = null;
        try {
            await installUpdate(handle, (p) => (pct = p));
            // relaunch() replaces the process; if we're still here it succeeded
            // but the OS hasn't swapped yet.
        } catch (e) {
            error = e instanceof Error ? e.message : String(e);
            phase = 'error';
        }
    }
</script>

{#if phase !== 'idle' && !dismissed}
    <div class="update-bar" class:err={phase === 'error'} role="status">
        {#if phase === 'available'}
            <span class="up-dot"></span>
            <span
                >Update available — <b>ISC MingoROS {version}</b>. Restart to
                install.</span
            >
            <div class="up-actions">
                <button type="button" class="up-ghost" onclick={() => (dismissed = true)}
                    >Later</button
                >
                <button type="button" class="up-go" onclick={() => void install()}
                    >Install &amp; restart</button
                >
            </div>
        {:else if phase === 'installing'}
            <span class="up-dot busy"></span>
            <span
                >Installing <b>{version}</b>…{pct != null ? ` ${pct}%` : ''} — the
                app will restart.</span
            >
        {:else if phase === 'error'}
            <span class="up-dot"></span>
            <span>Update failed: {error}</span>
            <div class="up-actions">
                <button type="button" class="up-ghost" onclick={() => (dismissed = true)}
                    >Dismiss</button
                >
                <button type="button" class="up-go" onclick={() => void install()}
                    >Retry</button
                >
            </div>
        {/if}
    </div>
{/if}
