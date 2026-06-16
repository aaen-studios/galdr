import { useState, useRef, useEffect } from "react";
import { AnimatePresence, motion } from "framer-motion";
import type { ReactNode } from "react";
import { useGaldrStore } from "./store";
import "./transitions.css";

export type TransitionStyle =
  | "none"
  | "rune-dissolve"
  | "terminal-scroll"
  | "runic-portal"
  | "ink-ripple"
  | "angular-carve";

const OVERLAY_DURATION = 300;
const RUNES = "ᚠᚢᚦᚨᚱᚲᚷᚹᚺᚾᛁᛃᛇᛈᛉᛊᛏᛒᛖᛗᛚᛝᛟᛞ";
const RUNES_SHORT = "ᚠᚢᚦᚨᚱᚲᚷᚹᚺᚾᛁᛃᛇᛈᛉᛊᛏᛒᛖᛗᛚ";

/* ==========================================
   OVERLAYS
   ========================================== */

function RuneDissolveOverlay() {
  const layers = useRef(
    [0, 1, 2].map((layer) => {
      const count = 20 - layer * 4;
      return Array.from({ length: count }, () => ({
        char: RUNES[Math.floor(Math.random() * RUNES.length)],
        left: Math.random() * 100,
        delay: Math.random() * 0.12 + layer * 0.03,
        duration: 0.25 + Math.random() * 0.2 + layer * 0.06,
        size: 12 + Math.random() * 16 - layer * 3,
        alpha: 0.2 + Math.random() * 0.3 + layer * 0.1,
      }));
    }),
  ).current;

  return (
    <div className="to-rune-dissolve">
      {layers.map((items, li) => (
        <div key={li} className="rd-layer" style={{ zIndex: li }}>
          {items.map((r, i) => (
            <span
              key={i}
              className="rd-rune"
              style={{
                left: `${r.left}%`,
                fontSize: r.size,
                opacity: r.alpha,
                animationDelay: `${r.delay}s`,
                animationDuration: `${r.duration}s`,
              }}
            >
              {r.char}
            </span>
          ))}
        </div>
      ))}
    </div>
  );
}

function TerminalScrollOverlay() {
  const lines = [
    { rune: "ᚷ", text: "galdr init ... ok" },
    { rune: "ᚲ", text: "rune engine  ... loaded" },
    { rune: "ᛏ", text: "conversion modules ... ready" },
    { rune: "ᛟ", text: "page transition ... complete" },
  ];

  return (
    <div className="to-terminal-scroll">
      {lines.map((l, i) => (
        <div
          key={i}
          className="ts-line"
          style={{
            animationDelay: `${i * 0.06}s`,
            animationDuration: `${0.08 + Math.random() * 0.06}s`,
          }}
        >
          <span className="ts-rune-char">[{l.rune}]</span> {l.text}
        </div>
      ))}
      <span
        className="ts-cursor"
        style={{ animationDelay: `${lines.length * 0.06 + 0.18}s` }}
      />
    </div>
  );
}

function RunicPortalOverlay() {
  const rings = useRef(
    [1, 2, 3].map((ring) => {
      const radius = 50 + ring * 35;
      const count = 14 + ring * 4;
      const runes = Array.from({ length: count }, () =>
        RUNES_SHORT[Math.floor(Math.random() * RUNES_SHORT.length)],
      );
      return { radius, count, runes, ring };
    }),
  ).current;

  return (
    <div className="to-runic-portal">
      <div className="rp-center" />
      {rings.map((r) => (
        <div
          key={r.ring}
          className="rp-ring-container"
          style={{
            animation: `rp-ring-in ${0.2 + r.ring * 0.04}s ease-out ${(r.ring - 1) * 0.03}s forwards`,
          }}
        >
          <div
            className="rp-ring-rotator"
            style={{
              animation: `rp-spin-${r.ring % 2 === 0 ? "cw" : "ccw"} ${0.6 + r.ring * 0.3}s linear infinite`,
            }}
          >
            {r.runes.map((ch, j) => {
              const angle = (360 / r.count) * j;
              return (
                <span
                  key={j}
                  className="rp-rune"
                  style={{
                    transform: `rotate(${angle}deg) translateY(-${r.radius}px)`,
                    fontSize: Math.max(10, 16 - r.ring * 2),
                    color: `rgba(200,200,200,${0.4 + r.ring * 0.15})`,
                  }}
                >
                  {ch}
                </span>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

function InkRippleOverlay() {
  const rings = useRef(
    Array.from({ length: 5 }, (_, i) => {
      const count = 16 + i * 6;
      const runes = Array.from({ length: count }, () =>
        RUNES_SHORT[Math.floor(Math.random() * RUNES_SHORT.length)],
      );
      return { runes, count, idx: i };
    }),
  ).current;

  return (
    <div className="to-ink-ripple">
      {rings.map((r) => (
        <div
          key={r.idx}
          className="ir-ring"
          style={{
            animation: `ir-pulse ${0.18 + r.idx * 0.03}s ease-out ${0.008 * r.idx}s forwards`,
          }}
        >
          {r.runes.map((ch, j) => {
            const angle = (360 / r.count) * j;
            const radius = 25 + r.idx * 28;
            return (
              <span
                key={j}
                className="ir-rune"
                style={{
                  transform: `rotate(${angle}deg) translateY(-${radius}px)`,
                  fontSize: Math.max(8, 14 - r.idx * 1.5),
                }}
              >
                {ch}
              </span>
            );
          })}
        </div>
      ))}
    </div>
  );
}

function AngularCarveOverlay() {
  const sparks = useRef(
    Array.from({ length: 28 }, () => ({
      rune: RUNES_SHORT[Math.floor(Math.random() * RUNES_SHORT.length)],
      top: 5 + Math.random() * 90,
      left: 5 + Math.random() * 90,
      delay: Math.random() * 0.12,
      duration: 0.1 + Math.random() * 0.1,
    })),
  ).current;

  return (
    <div className="to-angular-carve">
      <div className="ac-blade-fwd">
        {sparks.slice(0, 14).map((s, i) => (
          <span
            key={i}
            className="ac-rune"
            style={{
              top: `${s.top}%`,
              left: `${s.left}%`,
              animationDelay: `${s.delay}s`,
              animationDuration: `${s.duration}s`,
            }}
          >
            {s.rune}
          </span>
        ))}
      </div>
      <div className="ac-blade-rev">
        {sparks.slice(14).map((s, i) => (
          <span
            key={i}
            className="ac-rune"
            style={{
              top: `${s.top}%`,
              left: `${s.left}%`,
              animationDelay: `${s.delay}s`,
              animationDuration: `${s.duration}s`,
            }}
          >
            {s.rune}
          </span>
        ))}
      </div>
      <div className="ac-flash" />
    </div>
  );
}

/* ==========================================
   TRANSITION DEFINITIONS
   ========================================== */

type Anim = Record<string, any>;

interface TransitionDef {
  label: string;
  rune: string;
  initial: Anim;
  animate: Anim;
  exit: Anim;
}

export const TRANSITIONS: Record<TransitionStyle, TransitionDef> = {
  "none": {
    label: "None",
    rune: "—",
    initial: {},
    animate: {},
    exit: {},
  },
  "rune-dissolve": {
    label: "Rune Dissolve",
    rune: "ᚲ",
    initial: { opacity: 0 },
    animate: { opacity: 1, transition: { duration: 0.08 } },
    exit: { opacity: 0, transition: { duration: 0.08 } },
  },
  "terminal-scroll": {
    label: "Terminal Scroll",
    rune: "ᛏ",
    initial: { opacity: 0, y: 16 },
    animate: { opacity: 1, y: 0, transition: { duration: 0.2, ease: [0.19, 1, 0.22, 1] } },
    exit: { opacity: 0, y: -16, transition: { duration: 0.12, ease: "easeIn" } },
  },
  "runic-portal": {
    label: "Runic Portal",
    rune: "ᚷ",
    initial: { opacity: 0 },
    animate: { opacity: 1, transition: { duration: 0.08 } },
    exit: { opacity: 0, transition: { duration: 0.08 } },
  },
  "ink-ripple": {
    label: "Ink Ripple",
    rune: "ᛟ",
    initial: { opacity: 0 },
    animate: { opacity: 1, transition: { duration: 0.08 } },
    exit: { opacity: 0, transition: { duration: 0.08 } },
  },
  "angular-carve": {
    label: "Angular Carve",
    rune: "ᛉ",
    initial: { opacity: 0 },
    animate: { opacity: 1, transition: { duration: 0.08 } },
    exit: { opacity: 0, transition: { duration: 0.08 } },
  },
};

export const TRANSITION_OPTIONS = Object.entries(TRANSITIONS).map(
  ([value, t]) => ({ value, label: `${t.rune} ${t.label}` }),
);

export const DEFAULT_TRANSITION: TransitionStyle = "none";

const OVERLAY_MAP: Partial<Record<TransitionStyle, () => React.JSX.Element>> = {
  "rune-dissolve": RuneDissolveOverlay,
  "terminal-scroll": TerminalScrollOverlay,
  "runic-portal": RunicPortalOverlay,
  "ink-ripple": InkRippleOverlay,
  "angular-carve": AngularCarveOverlay,
};

interface Props {
  style: TransitionStyle;
  pageKey: string;
  children: ReactNode;
}

export default function PageTransition({ style, pageKey, children }: Props) {
  const [showOverlay, setShowOverlay] = useState(false);
  const prevKey = useRef(pageKey);
  const prevTestSignal = useRef(0);
  const overlayTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const testSignal = useGaldrStore((s) => s.testTransitionSignal);

  useEffect(() => {
    const keyChanged = prevKey.current !== pageKey;
    const testTriggered = testSignal !== prevTestSignal.current && testSignal > 0;

    if (keyChanged) prevKey.current = pageKey;
    if (testTriggered) prevTestSignal.current = testSignal;

    if ((keyChanged || testTriggered) && style !== "none") {
      setShowOverlay(true);
      clearTimeout(overlayTimer.current);
      overlayTimer.current = setTimeout(() => setShowOverlay(false), OVERLAY_DURATION);
    }
    return () => clearTimeout(overlayTimer.current);
  }, [pageKey, testSignal, style]);

  const t = TRANSITIONS[style];
  const OverlayComponent = OVERLAY_MAP[style];

  return (
    <div className="page-transition-wrap">
      <AnimatePresence mode="wait">
        <motion.div
          key={pageKey}
          initial={t.initial}
          animate={t.animate}
          exit={t.exit}
        >
          {children}
        </motion.div>
      </AnimatePresence>

      {showOverlay && OverlayComponent && (
        <div className="to-overlay">
          <OverlayComponent />
        </div>
      )}
    </div>
  );
}
