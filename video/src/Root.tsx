import React from "react";
import { Composition } from "remotion";
import { loadFont as loadCascadia } from "@remotion/google-fonts/CascadiaMono";
import { loadFont as loadRunic } from "@remotion/google-fonts/NotoSansRunic";
import { GaldrReel } from "./GaldrReel";
import { Boot } from "./scenes/Boot";
import { LogoReveal } from "./scenes/LogoReveal";
import { Convert } from "./scenes/Convert";
import { Transcribe } from "./scenes/Transcribe";
import { Forge } from "./scenes/Forge";
import { LogoClose } from "./scenes/LogoClose";
import { VIDEO, TIMING, TOTAL_DURATION } from "./theme";

// Load webfonts so renders are reproducible across machines. Cascadia Mono is the
// primary face; Noto Sans Runic is a fallback the browser uses automatically for
// Elder Futhark glyphs (the host OS monospace stack may lack the Runic block).
// loadFont() returns the resolved family name; we ignore it here and reference
// the families via FONT_STACK in theme.ts.
loadCascadia();
loadRunic();

/**
 * Root composition registration. The master reel is the primary output; each
 * scene is also registered as its own composition so it can be previewed (and
 * rendered) in isolation in Remotion Studio while building.
 */
export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="GaldrReel"
        component={GaldrReel}
        durationInFrames={TOTAL_DURATION}
        fps={VIDEO.fps}
        width={VIDEO.width}
        height={VIDEO.height}
      />

      <Composition id="Boot" component={Boot} durationInFrames={TIMING.boot.durationInFrames} fps={VIDEO.fps} width={VIDEO.width} height={VIDEO.height} />
      <Composition id="LogoReveal" component={LogoReveal} durationInFrames={TIMING.logoReveal.durationInFrames} fps={VIDEO.fps} width={VIDEO.width} height={VIDEO.height} />
      <Composition id="Convert" component={Convert} durationInFrames={TIMING.convert.durationInFrames} fps={VIDEO.fps} width={VIDEO.width} height={VIDEO.height} />
      <Composition id="Transcribe" component={Transcribe} durationInFrames={TIMING.transcribe.durationInFrames} fps={VIDEO.fps} width={VIDEO.width} height={VIDEO.height} />
      <Composition id="Forge" component={Forge} durationInFrames={TIMING.forge.durationInFrames} fps={VIDEO.fps} width={VIDEO.width} height={VIDEO.height} />
      <Composition id="LogoClose" component={LogoClose} durationInFrames={TIMING.logoClose.durationInFrames} fps={VIDEO.fps} width={VIDEO.width} height={VIDEO.height} />
    </>
  );
};
