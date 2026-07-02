<!--
    RES remote-e-stop bar. Its own emphasis: uses /res/status for the
    big word + ring tone, and the separate /res/go boolean topic for
    the inline pill. RES tone: estop/emergency/timeout/fail -> no;
    go/ok/ready -> go; else neutral. Danger forces no.
-->
<script lang="ts">
    import type { TopicSnapshot } from '../types';
    import { classifySignal, extractWord } from '../model';

    interface Props {
        byTopic: Record<string, TopicSnapshot>;
    }

    const { byTopic }: Props = $props();

    interface ResView {
        word: string;
        tone: 'go' | 'no' | 'neu';
    }

    const res = $derived.by<ResView>(() => {
        const r = byTopic['/res/status'];
        let word = '—';
        let tone: 'go' | 'no' | 'neu' = 'neu';
        if (r && r.state === 'ok') {
            word = extractWord(r.value) || r.value || '—';
            const w = word.toLowerCase();
            if (/estop|emergency|timeout|fail/.test(w)) tone = 'no';
            else if (/\bgo\b|^ok$|ready/.test(w)) tone = 'go';
            else tone = 'neu';
            if (r.danger) tone = 'no';
        } else if (r && r.state === 'unavailable') {
            word = 'unavailable';
        } else {
            word = 'waiting…';
        }
        return { word, tone };
    });

    interface PillView {
        cls: string;
        text: string;
    }

    const pill = $derived.by<PillView>(() => {
        const g = byTopic['/res/go'];
        if (g && g.state === 'ok') {
            const gw = extractWord(g.value) || g.value || '';
            const gc = classifySignal('go', gw); // GO -> good, NO-GO -> neutral
            const cls = gc === 'good' ? 'go' : gc === 'bad' ? 'no' : '';
            return { cls, text: 'RES GO · ' + gw };
        }
        return { cls: '', text: 'RES GO · —' };
    });
</script>

<section class="res {res.tone}">
    <div class="ring"><i></i></div>
    <div class="label">
        <span class="badge">Remote e-stop</span>
        <span class="name">Remote Emergency System · <b>/res/status</b></span>
    </div>
    <span class="grow"></span>
    <div class="readout">
        <span class="word">{res.word}</span>
        <span class="gopill {pill.cls}"
            ><span class="led"></span><span class="t">{pill.text}</span></span
        >
    </div>
</section>
