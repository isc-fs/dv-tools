<!--
    Key-facts strip: AS state / DV pipeline / Mission. Each cell reads
    its snapshot, extracts the compact word, derives a good/bad/idle
    tone (stale overrides tone), and shows the age. `idle` is added
    whenever the topic isn't a live "ok" sample.
-->
<script lang="ts">
    import type { TopicSnapshot } from '../types';
    import { classifyTopicWord, extractWord, fmtAge } from '../model';

    interface Props {
        byTopic: Record<string, TopicSnapshot>;
    }

    const { byTopic }: Props = $props();

    interface FactDef {
        topic: string;
        k: string;
    }
    const DEFS: FactDef[] = [
        { topic: '/assi/state', k: 'AS state' },
        { topic: '/dv/status', k: 'DV pipeline' },
        { topic: '/ami/mission', k: 'Mission' },
    ];

    interface Fact {
        k: string;
        word: string;
        age: string;
        tone: 'go' | 'no' | 'stale' | 'idle';
        idle: boolean;
    }

    const facts = $derived.by<Fact[]>(() =>
        DEFS.map((def): Fact => {
            const r = byTopic[def.topic];
            let word = '—';
            let age = '';
            let tone: 'go' | 'no' | 'stale' | 'idle' = 'idle';
            if (r && r.state === 'ok') {
                word = extractWord(r.value) || r.value || '—';
                age = fmtAge(r.age_ms);
                tone = classifyTopicWord(r.value, r.danger);
                if (!r.fresh) tone = 'stale';
            } else if (r && r.state === 'unavailable') {
                word = 'unavailable';
            } else {
                word = 'waiting…';
            }
            const idle = !r || r.state !== 'ok';
            return { k: def.k, word, age, tone, idle };
        }),
    );
</script>

<section class="facts">
    {#each facts as f (f.k)}
        <div class="fact {f.tone}" class:idle={f.idle}>
            <div class="k"><span class="dot"></span>{f.k}</div>
            <div class="val">{f.word}</div>
            <div class="age num">{f.age ? 'age ' + f.age : ''}</div>
        </div>
    {/each}
</section>
