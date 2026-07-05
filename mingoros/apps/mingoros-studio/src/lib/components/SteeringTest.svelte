<!--
    Steering actuation self-test (#92) — the /activate_steering counterpart to
    Force-EBS. A guarded button (locked until the stands interlock is armed)
    opens a confirmation; the modal's explicit "Activate" fires the uDV's
    /activate_steering (std_srvs/SetBool). Watch /steering/feedback +
    /steering_angle in the echo tab to confirm the actuator actually moved.
-->
<script lang="ts">
    import { activateSteering } from '../api';
    import type { EbsResult } from '../types';

    interface Props {
        /** Stands interlock (#60): locked until armed. */
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
            const r = await activateSteering(engage);
            result = r;
            if (r.success) engaged = engage;
            else error = r.message || 'the uDV reported the service failed';
        } catch (e) {
            error = e instanceof Error ? e.message : String(e);
        }
        busy = false;
    }

    function close(): void {
        if (!busy) open = false;
    }
    function onKeydown(e: KeyboardEvent): void {
        if (open && e.key === 'Escape') close();
    }
</script>

<svelte:window onkeydown={onKeydown} />

<button
    type="button"
    class="ebs-btn steer-btn"
    class:armed={engaged}
    class:locked={!armed && !engaged}
    disabled={!armed && !engaged}
    onclick={() => (open = true)}
    title={armed || engaged
        ? 'Steering self-test — car-on-stands checkup'
        : 'LOCKED — arm the stands interlock to enable actuation'}
>
    <span class="ebs-dot"></span>{engaged ? 'STEER ON' : armed ? 'STEER TEST' : 'STEER 🔒'}
</button>

{#if open}
    <div class="modal-scrim" role="presentation">
        <div class="modal" role="dialog" aria-modal="true" aria-label="Steering self-test">
            <h2>Steering self-test <span class="tag">car-on-stands checkup</span></h2>
            <p class="modal-warn">
                This drives the <b>steering actuator</b> via
                <code>/activate_steering</code>. Only with the car
                <b>jacked up / on stands</b>. Watch <code>/steering/feedback</code>
                and <code>/steering_angle</code> in the <b>Topic echo</b> tab to
                confirm it moved.
            </p>

            {#if engaged}
                <p class="modal-state armed">
                    <span class="ebs-dot"></span>Steering is <b>ACTIVE</b>.
                    Deactivate to return to normal.
                </p>
            {/if}
            {#if error}
                <p class="modal-result bad">{error}</p>
            {:else if result}
                <p class="modal-result">{result.message}</p>
            {/if}

            <div class="modal-actions">
                <button type="button" class="btn-ghost" disabled={busy} onclick={close}
                    >Close</button
                >
                {#if engaged}
                    <button
                        type="button"
                        class="btn-release"
                        disabled={busy}
                        onclick={() => void call(false)}>{busy ? 'Deactivating…' : 'Deactivate'}</button
                    >
                {:else}
                    <button
                        type="button"
                        class="btn-danger"
                        disabled={busy || !armed}
                        onclick={() => void call(true)}>{busy ? 'Activating…' : 'Activate steering'}</button
                    >
                {/if}
            </div>
        </div>
    </div>
{/if}
