<!--
    Reusable grouped checklist (used twice — Safety chain and Drive
    readiness). Renders one PASS/FAIL/HOLD row per ordered name with
    the ✓/✗/– marker, the raw value, and a per-group tally. When no
    /debug signals have arrived yet it shows a "waiting…" placeholder.
-->
<script lang="ts">
    import type { ParsedSignal, RowKind } from '../types';
    import {
        NICE,
        classifySignal,
        classToKind,
        markerGlyph,
        statusWord,
    } from '../model';

    interface Props {
        /** Card heading, e.g. "Safety chain / AS arming". */
        title: string;
        /** Ordered signal names to render as rows. */
        names: string[];
        /** Lower-cased name -> parsed signal lookup. */
        map: Record<string, ParsedSignal>;
        /** True until any /debug signal has been parsed. */
        waiting: boolean;
    }

    const { title, names, map, waiting }: Props = $props();

    interface Row {
        key: string;
        name: string;
        nice: string | undefined;
        val: string;
        kind: RowKind;
    }

    const rows = $derived.by<Row[]>(() =>
        names.map((name): Row => {
            const s = map[name.toLowerCase()];
            const kind: RowKind = s
                ? classToKind(classifySignal(s.name, s.val))
                : 'hold';
            return {
                key: name,
                name: s ? s.name : name,
                nice: NICE[name],
                val: s ? s.val : '—',
                kind,
            };
        }),
    );

    const tally = $derived.by(() => {
        let pass = 0;
        let fail = 0;
        let tot = 0;
        for (const r of rows) {
            tot++;
            if (r.kind === 'pass') pass++;
            if (r.kind === 'fail') fail++;
        }
        return { pass, fail, tot };
    });

    const tallyClass = $derived(tally.fail > 0 ? 'warn' : 'ok');
    const tallyText = $derived(
        tally.fail > 0
            ? tally.fail + ' FAIL · ' + tally.pass + '/' + tally.tot + ' ok'
            : tally.pass + '/' + tally.tot + ' pass',
    );
</script>

<div class="card">
    <h3>
        {title}
        <span class="tally {waiting ? '' : tallyClass}">
            {waiting ? '' : tallyText}
        </span>
    </h3>
    <ul class="check">
        {#if waiting}
            <li><div class="waiting">waiting for /debug…</div></li>
        {:else}
            {#each rows as row (row.key)}
                <li class={row.kind}>
                    <div class="mark-cell">
                        <span class="marker">{markerGlyph(row.kind)}</span>
                    </div>
                    <div class="li-name">
                        {row.name}{#if row.nice}<small>{row.nice}</small>{/if}
                    </div>
                    <div class="li-status">
                        <span class="li-val">{row.val}</span>
                        <span class="li-word">{statusWord(row.kind)}</span>
                    </div>
                </li>
            {/each}
        {/if}
    </ul>
</div>
