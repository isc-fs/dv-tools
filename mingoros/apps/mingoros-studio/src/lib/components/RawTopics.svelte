<!--
    Subordinate, collapsible raw-topic table (the 7 priority topics)
    plus the legend/footer. Row classing: danger / stale / no-sub /
    waiting, each with its own tag. The footer restates the PASS / FAIL
    / HOLD key and the watchdog + poll cadence.
-->
<script lang="ts">
    import type { Meta, TopicSnapshot } from '../types';
    import { fmtAge } from '../model';

    interface Props {
        rows: TopicSnapshot[];
        meta: Meta;
    }

    const { rows, meta }: Props = $props();

    interface RawRow {
        key: string;
        label: string;
        topic: string;
        cls: string;
        val: string;
        age: string;
        tag: string; // '', 'danger', 'stale', 'no sub', 'waiting'
        hint: string; // QoS-mismatch explainer for silent topics (#90)
    }

    // Contract QoS per priority topic — a topic on the graph but silent is the
    // signature QoS-mismatch trap: a reader that requests RELIABLE won't match a
    // BEST_EFFORT publisher, and one that requests TRANSIENT_LOCAL won't match a
    // VOLATILE one — either delivers zero samples, silently (#90).
    const QOS_HINT: Record<string, string> = {
        '/assi/state': 'BEST_EFFORT',
        '/as_state': 'BEST_EFFORT',
        '/dv/status': 'RELIABLE + TRANSIENT_LOCAL (latched)',
        '/res/status': 'BEST_EFFORT',
        '/res/go': 'BEST_EFFORT',
        '/ami/mission': 'BEST_EFFORT',
        '/debug': 'RELIABLE',
    };
    function qosExplainer(topic: string): string {
        const q = QOS_HINT[topic];
        return `No data — if the publisher is live, suspect a QoS mismatch.${
            q ? ` Contract QoS: ${q}.` : ''
        } A reader that requests RELIABLE won't match a BEST_EFFORT publisher (and TRANSIENT_LOCAL won't match VOLATILE) — zero samples, silently.`;
    }

    const rawRows = $derived.by<RawRow[]>(() =>
        rows.map((r): RawRow => {
            let cls = '';
            let tag = '';
            let val = '';
            let age = '';
            let hint = '';
            if (r.state === 'ok') {
                val = r.value ?? '';
                age = fmtAge(r.age_ms);
                if (r.danger) {
                    cls = 'danger';
                    tag = 'danger';
                } else if (!r.fresh) {
                    cls = 'stale';
                    tag = 'stale';
                }
            } else if (r.state === 'unavailable') {
                cls = 'muted';
                val = 'unavailable';
                tag = 'no sub';
                hint = qosExplainer(r.topic);
            } else {
                cls = 'muted';
                val = 'waiting…';
                tag = 'waiting';
                hint = qosExplainer(r.topic);
            }
            return { key: r.topic, label: r.label, topic: r.topic, cls, val, age, tag, hint };
        }),
    );

    const cnt = $derived(
        (rows.length ? rows.length : 7) + ' priority topics',
    );

    const footNote = $derived(
        'watchdog ' +
            (meta.watchdog_s ?? 1.5) +
            ' s · poll 250 ms' +
            (meta.error ? ' · ' + meta.error : ''),
    );
</script>

<details class="raw">
    <summary
        ><span class="chev"></span>Raw topic snapshot
        <span class="cnt">{cnt}</span></summary
    >
    <div class="tbl-scroll">
        <table>
            <thead
                ><tr>
                    <th style="width:120px">signal</th>
                    <th style="width:150px">topic</th>
                    <th>value</th>
                    <th style="width:90px;text-align:right">age</th>
                </tr></thead
            >
            <tbody>
                {#each rawRows as r (r.key)}
                    <tr class={r.cls} title={r.hint || undefined}>
                        <td class="lab">{r.label}</td>
                        <td class="top">{r.topic}</td>
                        <td class="val"
                            >{r.val}{#if r.hint}<span class="qos-hint" title={r.hint}
                                    >QoS?</span
                                >{/if}</td
                        >
                        <td class="age num"
                            >{r.age}
                            {#if r.tag}<span class="tag">{r.tag}</span>{/if}</td
                        >
                    </tr>
                {/each}
            </tbody>
        </table>
    </div>
</details>

<div class="foot">
    <span class="dotk k-pass"><span class="sw"></span>PASS — interlock satisfied</span>
    <span class="dotk k-fail"><span class="sw"></span>FAIL — blocking / fault</span>
    <span class="dotk k-hold"
        ><span class="sw"></span>— expected-off on a stopped car</span
    >
    <span class="grow" style="flex:1"></span>
    <span>{footNote}</span>
</div>
