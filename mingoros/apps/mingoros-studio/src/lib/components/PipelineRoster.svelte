<!--
    Pipeline stage-up roster (#85) — "what's missing".

    Classifies the live discovered topics into DV pipeline stages and shows each
    PRESENT / ABSENT, so when only the uDV agent is bridging (the exact bench
    trap: a blank board because the main pipeline isn't launched) the operator
    reads "DV pipeline NOT launched — only the uDV agent is bridging" instead of
    an empty screen. Best-effort name patterns; refine as the pipeline's real
    topic names are pinned down.
-->
<script lang="ts">
    import { onMount } from 'svelte';
    import type { TopicInfo } from '../types';
    import { listTopics } from '../api';

    interface Props {
        /** Live data actually arriving from the car. A discovered topic can be
         *  just the app's own subscription, so without data the "present" stages
         *  would be a false positive — gate on this. */
        receiving: boolean;
    }
    const { receiving }: Props = $props();

    const POLL_MS = 1500; // topic membership changes slowly

    // The uDV / safety surface the micro-ROS agent bridges (contract topics).
    const UDV_TOPICS = new Set([
        '/assi/state', '/as_state', '/dv/status', '/res/status', '/res/go',
        '/ami/mission', '/debug', '/force_ebs', '/activate_steering',
    ]);
    // Our own node + ROS built-ins — never counted as pipeline.
    const BUILTIN = /mingoros|parameter_events|rosout|\/clock$/i;
    // Best-effort per-stage topic-name patterns.
    const STAGE_DEFS: { stage: string; re: RegExp }[] = [
        { stage: 'Perception', re: /cone|percept|lidar|camera|cloud|detect|vision/i },
        { stage: 'SLAM / state', re: /slam|pose|odom|localiz|\bmap\b|state_est/i },
        { stage: 'Planning', re: /plan|path|traj|centerline|raceline|waypoint/i },
        { stage: 'Control', re: /control|\bctrl\b|cmd_vel|steer|throttle|\bbrake\b|actuat/i },
    ];

    interface Stage {
        stage: string;
        present: boolean;
        found: string[];
    }

    let topics = $state<TopicInfo[]>([]);
    let reachable = $state<boolean>(true);

    // Without live data, a discovered topic is just the app's own subscription
    // (no publisher) — so classify nothing as "present" until data flows.
    const names = $derived(
        receiving ? topics.map((t) => t.name).filter((n) => n && !BUILTIN.test(n)) : [],
    );
    const udvFound = $derived(names.filter((n) => UDV_TOPICS.has(n)));
    const pipelineNames = $derived(names.filter((n) => !UDV_TOPICS.has(n)));
    const stages = $derived<Stage[]>(
        STAGE_DEFS.map((d) => {
            const found = pipelineNames.filter((n) => d.re.test(n));
            return { stage: d.stage, present: found.length > 0, found };
        }),
    );
    const pipelineUp = $derived(pipelineNames.length > 0);
    const upCount = $derived(stages.filter((s) => s.present).length);

    const headline = $derived.by(() => {
        if (!reachable) return 'DDS query failed — is the DV PC reachable / the link up?';
        // Discovery can list topics that are only the app's own subscriptions
        // (no publisher). Until data actually flows, say so plainly instead of
        // implying the pipeline is present.
        if (!receiving)
            return 'DDS is up, but no data is arriving — link down, or the pipeline / uDV isn’t running.';
        if (!pipelineUp)
            return 'DV pipeline NOT launched — only the uDV agent is bridging.';
        return `DV pipeline up — ${upCount}/${stages.length} stages publishing.`;
    });
    const headlineCls = $derived(
        !reachable || !receiving ? 'bad' : !pipelineUp ? 'warn' : 'ok',
    );

    async function refresh(): Promise<void> {
        try {
            topics = await listTopics();
            reachable = true;
        } catch {
            reachable = false;
            topics = [];
        }
    }
    onMount(() => {
        void refresh();
        const id = setInterval(() => void refresh(), POLL_MS);
        return () => clearInterval(id);
    });
</script>

<section class="roster">
    <div class="roster-head">
        <span class="roster-title">PIPELINE</span>
        <span class="roster-headline {headlineCls}">{headline}</span>
    </div>
    <div class="roster-stages">
        <div class="stage" class:present={udvFound.length > 0}>
            <span class="stage-dot"></span>
            <span class="stage-name">uDV / safety <em>(agent)</em></span>
            <span class="stage-state">{udvFound.length ? `${udvFound.length} topics` : 'absent'}</span>
        </div>
        {#each stages as s (s.stage)}
            <div class="stage" class:present={s.present}>
                <span class="stage-dot"></span>
                <span class="stage-name">{s.stage}</span>
                <span class="stage-state" title={s.found.join('\n')}>
                    {s.present ? `${s.found.length} topics` : 'absent'}
                </span>
            </div>
        {/each}
    </div>
</section>
