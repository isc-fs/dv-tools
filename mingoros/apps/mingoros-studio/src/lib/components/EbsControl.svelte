<!--
    Force-EBS control — a guarded actuation for a car-on-stands checkup.

    The bar button opens a confirmation modal; only the modal's explicit
    "Engage EBS" fires the uDV's /force_ebs service (std_srvs/SetBool). Once
    engaged the button glows and the modal offers "Release" (returns to normal).
    Deliberately not one-click: firing the emergency brake must be intentional.
-->
<script lang="ts">
    import { forceEbs } from '../api';
    import type { EbsResult } from '../types';

    interface Props {
        /** The stands interlock (#60): actuation is LOCKED until this is armed. */
        armed: boolean;
    }
    const { armed }: Props = $props();

    let open = $state<boolean>(false);
    let busy = $state<boolean>(false);
    let engaged = $state<boolean>(false);
    let result = $state<EbsResult | null>(null);
    let error = $state<string | null>(null);

    async function call(engage: boolean): Promise<void> {
        busy = true;
        error = null;
        try {
            const r = await forceEbs(engage);
            result = r;
            if (r.success) {
                engaged = engage;
            } else {
                error = r.message || 'the uDV reported the service failed';
            }
        } catch (e) {
            error = e instanceof Error ? e.message : String(e);
        }
        busy = false;
    }

    function close(): void {
        if (!busy) {
            open = false;
        }
    }

    function onKeydown(e: KeyboardEvent): void {
        if (open && e.key === 'Escape') {
            close();
        }
    }
</script>

<svelte:window onkeydown={onKeydown} />

<button
    type="button"
    class="ebs-btn"
    class:armed={engaged}
    class:locked={!armed && !engaged}
    disabled={!armed && !engaged}
    onclick={() => (open = true)}
    title={armed || engaged
        ? 'Force the Emergency Brake System — car-on-stands checkup'
        : 'LOCKED — arm the stands interlock (confirm the car is on stands) to enable actuation'}
>
    <span class="ebs-dot"></span>{engaged ? 'EBS ENGAGED' : armed ? 'EBS' : 'EBS 🔒'}
</button>

{#if open}
    <div class="modal-scrim" role="presentation">
        <div class="modal" role="dialog" aria-modal="true" aria-label="Force EBS">
            <h2>Force EBS <span class="tag">car-on-stands checkup</span></h2>
            <p class="modal-warn">
                This fires the <b>Emergency Brake System</b> actuators via
                <code>/force_ebs</code>. Only do this with the car
                <b>jacked up / on stands</b> — never on the ground.
            </p>

            {#if engaged}
                <p class="modal-state armed">
                    <span class="ebs-dot"></span>EBS is currently
                    <b>ENGAGED</b>. Release it to return the car to normal.
                </p>
            {/if}

            {#if error}
                <p class="modal-result bad">{error}</p>
            {:else if result}
                <p class="modal-result">{result.message}</p>
            {/if}

            <div class="modal-actions">
                <button
                    type="button"
                    class="btn-ghost"
                    disabled={busy}
                    onclick={close}>Close</button
                >
                {#if engaged}
                    <button
                        type="button"
                        class="btn-release"
                        disabled={busy}
                        onclick={() => void call(false)}
                    >
                        {busy ? 'Releasing…' : 'Release EBS (normal)'}
                    </button>
                {:else}
                    <button
                        type="button"
                        class="btn-danger"
                        disabled={busy || !armed}
                        onclick={() => void call(true)}
                    >
                        {busy ? 'Engaging…' : 'Engage EBS'}
                    </button>
                {/if}
            </div>
        </div>
    </div>
{/if}
