import React from "react";
import { AbsoluteFill, Audio, Sequence, staticFile } from "remotion";
import { VIDEO, TIMING, TOTAL_DURATION } from "./theme";
import { CrtOverlay } from "./components/CrtOverlay";
import { Boot } from "./scenes/Boot";
import { LogoReveal } from "./scenes/LogoReveal";
import { Convert } from "./scenes/Convert";
import { Transcribe } from "./scenes/Transcribe";
import { Forge } from "./scenes/Forge";
import { LogoClose } from "./scenes/LogoClose";
import { RuneDissolve, RunicPortal, InkRipple, AngularCarve } from "./components/Transitions";

/**
 * Master composition — sequences the six scenes and the transitions between them.
 *
 * Transitions live in their own short <Sequence> blocks that overlap the tail of
 * the outgoing scene so they wipe over it. Each scene is also registered
 * standalone in Root.tsx for isolated preview in Remotion Studio.
 */
export const GaldrReel: React.FC = () => {
  return (
    <AbsoluteFill style={{ background: "#000" }}>
      {/* --- Scenes --- */}
      <Sequence from={TIMING.boot.from} durationInFrames={TIMING.boot.durationInFrames} name="Boot">
        <Boot />
      </Sequence>

      <Sequence from={TIMING.logoReveal.from} durationInFrames={TIMING.logoReveal.durationInFrames} name="Logo Reveal">
        <LogoReveal />
      </Sequence>

      <Sequence from={TIMING.convert.from} durationInFrames={TIMING.convert.durationInFrames} name="Convert">
        <Convert />
      </Sequence>

      <Sequence from={TIMING.transcribe.from} durationInFrames={TIMING.transcribe.durationInFrames} name="Transcribe">
        <Transcribe />
      </Sequence>

      <Sequence from={TIMING.forge.from} durationInFrames={TIMING.forge.durationInFrames} name="Forge">
        <Forge />
      </Sequence>

      <Sequence from={TIMING.logoClose.from} durationInFrames={TIMING.logoClose.durationInFrames} name="Logo Close">
        <LogoClose />
      </Sequence>

      {/* --- Transitions (overlap scene tails by ~30 frames) --- */}
      {/* boot -> logo reveal: rune dissolve */}
      <Sequence from={TIMING.logoReveal.from - 24} durationInFrames={30} name="tx: rune-dissolve">
        <RuneDissolve durationInFrames={30} />
      </Sequence>
      {/* logo reveal -> convert: angular carve */}
      <Sequence from={TIMING.convert.from - 12} durationInFrames={18} name="tx: angular-carve">
        <AngularCarve durationInFrames={18} />
      </Sequence>
      {/* convert -> transcribe: ink ripple */}
      <Sequence from={TIMING.transcribe.from - 22} durationInFrames={30} name="tx: ink-ripple">
        <InkRipple durationInFrames={30} />
      </Sequence>
      {/* transcribe -> forge: runic portal */}
      <Sequence from={TIMING.forge.from - 24} durationInFrames={30} name="tx: runic-portal">
        <RunicPortal durationInFrames={30} />
      </Sequence>
      {/* forge -> logo close: angular carve */}
      <Sequence from={TIMING.logoClose.from - 12} durationInFrames={18} name="tx: angular-carve 2">
        <AngularCarve durationInFrames={18} />
      </Sequence>

      {/* --- Soundtrack --- */}
      {/* Royalty-free energetic synthwave / retro-80s bed (Pixabay Content License).
          Track is 163s; the reel is 36s, so we start ~6s into the track (into the
          driving section) and fade the tail out over the logo close. See README.
          If the file is absent, Remotion renders silently. */}
      <Audio
        src={staticFile("audio/soundtrack.mp3")}
        startFromFrames={Math.round(6 * VIDEO.fps)}
        volume={(f) => {
          // fade in over first second, hold, fade out over last 1.5s
          const fadeIn = Math.min(1, f / 60);
          const fadeOut = f < TOTAL_DURATION - 90 ? 1 : Math.max(0, 1 - (f - (TOTAL_DURATION - 90)) / 90);
          return 0.55 * fadeIn * fadeOut;
        }}
      />

      {/* --- Persistent CRT overlay over everything --- */}
      <CrtOverlay />
    </AbsoluteFill>
  );
};
