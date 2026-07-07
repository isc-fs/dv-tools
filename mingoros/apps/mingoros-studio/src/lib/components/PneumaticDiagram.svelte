<!--
    EBS pneumatic schematic (#feat-60) — the training-HTML diagram, driven LIVE.

    Shows what actuates during the EBS self-check: the two storage tanks, the two
    actuator valves (D1/D2), the AS SDC relay (D4), the brake line, and the
    caliper. What's live: the firing valve (from the EBS-init sub-state), the SDC
    relay (from /debug SDC), and brake-engaged (from EBS-armed). What's NOT on the
    wire: the numeric tank pressure — so tanks read OK / LOW / charging by state,
    not a bar number.
-->
<script lang="ts">
    import type { PneumaticView } from '../startup';

    interface Props {
        p: PneumaticView;
    }
    const { p }: Props = $props();

    // tank fill is state-only (no bar value on the wire)
    const FRAC: Record<PneumaticView['tanks'], number> = {
        good: 0.74,
        charging: 0.45,
        low: 0.16,
        unknown: 0,
    };
    const frac = $derived(FRAC[p.tanks]);
    const tankH = $derived(Math.round(104 * frac));
    const tankLabel = $derived(
        p.tanks === 'good'
            ? 'OK'
            : p.tanks === 'low'
              ? 'LOW'
              : p.tanks === 'charging'
                ? 'charging'
                : '—',
    );
</script>

<svg
    class="pn"
    viewBox="0 0 940 372"
    xmlns="http://www.w3.org/2000/svg"
    role="img"
    aria-label="EBS pneumatic schematic — live activation state"
>
    <defs>
        <linearGradient id="pnGood" x1="0" y1="1" x2="0" y2="0">
            <stop offset="0" stop-color="#1c8f4e" />
            <stop offset="1" stop-color="#3fe07f" />
        </linearGradient>
    </defs>

    <!-- storage tanks -->
    <text class="pn-glab" x="78" y="30" text-anchor="middle">Storage A1 · A5</text>
    <rect class="pn-tank" x="26" y="40" width="104" height="118" rx="11" />
    <rect
        class="pn-fill"
        class:good={p.tanks === 'good'}
        class:low={p.tanks === 'low'}
        x="34"
        y={150 - tankH}
        width="88"
        height={tankH}
        rx="5"
    />
    <text class="pn-gval" class:good={p.tanks === 'good'} class:low={p.tanks === 'low'} x="78" y="105" text-anchor="middle">{tankLabel}</text>

    <text class="pn-glab" x="78" y="204" text-anchor="middle">Storage A2 · A4</text>
    <rect class="pn-tank" x="26" y="214" width="104" height="118" rx="11" />
    <rect
        class="pn-fill"
        class:good={p.tanks === 'good'}
        class:low={p.tanks === 'low'}
        x="34"
        y={324 - tankH}
        width="88"
        height={tankH}
        rx="5"
    />
    <text class="pn-gval" class:good={p.tanks === 'good'} class:low={p.tanks === 'low'} x="78" y="279" text-anchor="middle">{tankLabel}</text>

    <!-- pipes tank -> valve -->
    <path class="pn-pipe" class:hot={p.firingD1} d="M130 99 H196" />
    <path class="pn-pipe" class:hot={p.firingD2} d="M130 273 H196" />

    <!-- actuator valves -->
    <circle class="pn-valve" class:firing={p.firingD1} cx="224" cy="99" r="24" />
    <path class="pn-vg" class:firing={p.firingD1} d="M212 87 L236 111 M236 87 L212 111" />
    <text class="pn-vlab" x="224" y="150" text-anchor="middle">EBS act 1 · D1</text>
    <circle class="pn-valve" class:firing={p.firingD2} cx="224" cy="273" r="24" />
    <path class="pn-vg" class:firing={p.firingD2} d="M212 261 L236 285 M236 261 L212 285" />
    <text class="pn-vlab" x="224" y="324" text-anchor="middle">EBS act 2 · D2</text>

    <!-- valve -> manifold -->
    <path class="pn-pipe" class:hot={p.firingD1} d="M248 99 H470" />
    <path class="pn-pipe" class:hot={p.firingD2} d="M248 273 H470" />
    <path class="pn-pipe" class:hot={p.braking} d="M470 99 V273" />

    <!-- AS SDC relay (D4) -->
    <text class="pn-sdclab" x="360" y="34" text-anchor="middle">AS SDC · D4</text>
    <rect class="pn-sdcbox" class:closed={p.sdcClosed} x="300" y="44" width="120" height="40" rx="8" />
    <line class="pn-sdcc" class:closed={p.sdcClosed} x1="318" y1="64" x2="352" y2="64" />
    <line class="pn-sdcc" class:closed={p.sdcClosed} x1="352" y1="64" x2="386" y2={p.sdcClosed ? 64 : 52} />
    <circle cx="352" cy="64" r="3" fill="#5b6675" />
    <line class="pn-sdcc" class:closed={p.sdcClosed} x1="386" y1="64" x2="402" y2="64" />
    <text class="pn-sdcstate" x="360" y="102" text-anchor="middle" fill={p.sdcClosed ? '#37d67a' : '#7e8a9a'}>
        {p.sdcClosed ? 'CLOSED' : 'OPEN'}
    </text>

    <!-- brake line -> caliper -->
    <path class="pn-bl" class:hot={p.braking} d="M470 186 H700" opacity={p.braking ? 1 : 0.3} />
    <text class="pn-vlab" x="590" y="168" text-anchor="middle">Brake line · 0x505 verdict</text>

    <!-- caliper + disc -->
    <text class="pn-vlab" x="772" y="120" text-anchor="middle">Brake</text>
    <circle class="pn-disc" class:braking={p.braking} cx="772" cy="186" r="54" />
    <circle class="pn-discin" cx="772" cy="186" r="30" />
    <rect class="pn-pad" class:on={p.braking} x="704" y="162" width="20" height="48" rx="3" transform={p.braking ? 'translate(16,0)' : 'translate(0,0)'} />
    <rect class="pn-pad" class:on={p.braking} x="820" y="162" width="20" height="48" rx="3" transform={p.braking ? 'translate(-16,0)' : 'translate(0,0)'} />
</svg>

<style>
    .pn {
        display: block;
        width: 100%;
        height: auto;
    }
    .pn-tank { fill: #0d131c; stroke: #33404f; stroke-width: 2; }
    .pn-fill { fill: #2b3a4a; transition: y 0.3s ease, height 0.3s ease, fill 0.3s; }
    .pn-fill.good { fill: url(#pnGood); }
    .pn-fill.low { fill: #7a2a30; }
    .pn-glab {
        fill: var(--ink-faint);
        font-family: var(--mono);
        font-size: 9px;
        font-weight: 600;
        letter-spacing: 0.08em;
        text-transform: uppercase;
    }
    .pn-gval { fill: var(--ink); font-family: var(--mono); font-size: 13px; font-weight: 700; }
    .pn-gval.good { fill: var(--go); }
    .pn-gval.low { fill: var(--no); }
    .pn-pipe {
        fill: none;
        stroke: #2a3542;
        stroke-width: 5;
        stroke-linecap: round;
        transition: stroke 0.25s, filter 0.25s;
    }
    .pn-pipe.hot { stroke: var(--no); filter: drop-shadow(0 0 5px rgba(255, 83, 71, 0.73)); }
    .pn-valve { fill: #111925; stroke: #3a4655; stroke-width: 2.5; transition: 0.2s; }
    .pn-valve.firing { fill: #3a1416; stroke: var(--no); filter: drop-shadow(0 0 9px rgba(255, 83, 71, 0.8)); }
    .pn-vg { stroke: #5b6675; stroke-width: 2.5; fill: none; transition: 0.2s; }
    .pn-vg.firing { stroke: #ffd0cc; }
    .pn-vlab {
        fill: var(--ink-dim);
        font-family: var(--mono);
        font-size: 9px;
        font-weight: 600;
        letter-spacing: 0.05em;
        text-transform: uppercase;
    }
    .pn-sdclab {
        fill: var(--ink-dim);
        font-family: var(--mono);
        font-size: 9px;
        font-weight: 600;
        letter-spacing: 0.06em;
        text-transform: uppercase;
    }
    .pn-sdcbox { fill: #0d131c; stroke: #3a4655; stroke-width: 2; transition: 0.2s; }
    .pn-sdcbox.closed { stroke: var(--go); filter: drop-shadow(0 0 7px rgba(55, 214, 122, 0.53)); }
    .pn-sdcc { stroke: #5b6675; stroke-width: 3; stroke-linecap: round; transition: 0.25s; }
    .pn-sdcc.closed { stroke: var(--go); }
    .pn-sdcstate { font-family: var(--mono); font-size: 10px; font-weight: 700; letter-spacing: 0.06em; }
    .pn-bl {
        fill: none;
        stroke: #2a3542;
        stroke-width: 6;
        stroke-linecap: round;
        transition: stroke 0.2s, opacity 0.2s, filter 0.2s;
    }
    .pn-bl.hot { stroke: var(--no); filter: drop-shadow(0 0 7px rgba(255, 83, 71, 0.8)); }
    .pn-disc { fill: none; stroke: #3d4a5a; stroke-width: 3; transition: 0.2s; }
    .pn-disc.braking { stroke: var(--no); filter: drop-shadow(0 0 8px rgba(255, 83, 71, 0.53)); }
    .pn-discin { fill: #0d131c; stroke: #28323f; stroke-width: 1.5; }
    .pn-pad { fill: #4a5666; stroke: #5b6675; stroke-width: 1; transition: transform 0.25s ease, fill 0.25s; }
    .pn-pad.on { fill: var(--no); }
</style>
