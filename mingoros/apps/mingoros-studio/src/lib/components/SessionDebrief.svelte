<!--
    Session debrief card (#55) — the "what just happened" summary shown on the
    Details tab. Reads the shared recorderStore, so it reflects whatever the
    board's record toggle captured. Empty until a session has been recorded.
-->
<script lang="ts">
    import { recorder, fmtT } from '../recorderStore.svelte';
</script>

{#if recorder.recording}
    <div class="debrief">
        <div class="db-title">SESSION RECORDING…</div>
        <div class="db-row">
            <span class="db-k">elapsed</span><span class="db-v">{fmtT(recorder.now)}</span>
            <span class="db-k">transitions</span><span class="db-v">{recorder.events.length}</span>
        </div>
        <div class="db-hint">Stop the recording from the board's record control to see the debrief.</div>
    </div>
{:else if recorder.events.length > 0}
    <div class="debrief">
        <div class="db-title">SESSION DEBRIEF</div>
        <div class="db-row">
            <span class="db-k">duration</span><span class="db-v">{fmtT(recorder.debrief.dur)}</span>
            <span class="db-k">transitions</span><span class="db-v">{recorder.debrief.transitions}</span>
            <span class="db-k">faults</span>
            <span class="db-v" class:bad={recorder.debrief.faultCount > 0}>
                {recorder.debrief.faultCount}{#if recorder.debrief.firstFault != null}
                    (first at {fmtT(recorder.debrief.firstFault)}){/if}
            </span>
        </div>
        {#if recorder.debrief.dwell.length}
            <div class="db-dwell">
                {#each recorder.debrief.dwell as [state, ms] (state)}
                    <span class="db-chip"><b>{state}</b> {fmtT(ms)}</span>
                {/each}
            </div>
        {/if}
    </div>
{:else}
    <div class="debrief db-empty">
        <div class="db-title">SESSION DEBRIEF</div>
        <div class="db-hint">
            No session recorded yet. Use <b>⏺ Record session</b> on the Go / No-Go board, then
            stop it to get a "what just happened" summary here.
        </div>
    </div>
{/if}
