import React from "react";

/**
 * Persistent CRT scanline overlay, ported from galdr's `.to-terminal-scroll::before`
 * scanline rule (src/transitions.css). Sits above all scene content at low opacity.
 *
 * Optional `vignette` adds a faint phosphor edge falloff for depth.
 */
export const CrtOverlay: React.FC<{ vignette?: boolean }> = ({ vignette = true }) => {
  return (
    <>
      <div
        style={{
          position: "absolute",
          inset: 0,
          zIndex: 40,
          pointerEvents: "none",
          background:
            "repeating-linear-gradient(0deg, transparent, transparent 2px, rgba(200,200,200,0.022) 2px, rgba(200,200,200,0.022) 4px)",
        }}
      />
      {vignette && (
        <div
          style={{
            position: "absolute",
            inset: 0,
            zIndex: 41,
            pointerEvents: "none",
            background:
              "radial-gradient(ellipse at center, transparent 55%, rgba(0,0,0,0.55) 100%)",
          }}
        />
      )}
    </>
  );
};
