<!--
    Session-recorder TOGGLE (#55). The capture + debrief now live in the shared
    recorderStore so this control sits on the board's gauge deck while the
    debrief card shows on the Details tab — both reading one source, capture
    running on every tab. This component is just the record/stop button + the
    live counter.
-->
<script lang="ts">
    import { recorder, fmtT } from '../recorderStore.svelte';
</script>

<section class="recorder">
    <div class="rec-head">
        <button
            type="button"
            class="rec-btn"
            class:on={recorder.recording}
            onclick={() => (recorder.recording ? recorder.stop() : recorder.start())}
        >
            {recorder.recording ? '■ Stop' : '⏺ Record session'}
        </button>
        {#if recorder.recording}
            <span class="rec-live"
                >● REC {fmtT(recorder.now)} · {recorder.events.length} transitions</span
            >
        {:else if recorder.events.length > 0}
            <span class="rec-done">✓ {recorder.events.length} · see Details</span>
        {/if}
    </div>
</section>
