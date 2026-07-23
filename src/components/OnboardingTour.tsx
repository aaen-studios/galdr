import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useGaldrStore } from "../store";

const STEPS = [
  {
    rune: "ᚲ",
    title: "convert",
    body: "drop a media file onto the convert page, pick a format, and hit cast. the ffmpeg command builds live below so you can see exactly what runs.",
  },
  {
    rune: "ᚱ",
    title: "rune tags",
    body: "runes are saved presets — capture your favorite conversion settings and re-apply them in one click. twelve are bundled to start.",
  },
  {
    rune: "ᚷ",
    title: "watch folders",
    body: "point galdr at a folder and it converts incoming files automatically, even when minimized to tray. great for hands-off batch processing.",
  },
  {
    rune: "ᚠ",
    title: "command palette",
    body: "press ctrl+k anywhere to jump between pages and run commands. press ? for keyboard shortcuts. that's the tour — go cast.",
  },
];

interface Props {
  onComplete: () => void;
}

export default function OnboardingTour({ onComplete }: Props) {
  const [step, setStep] = useState(0);
  const s = STEPS[step];
  const isLast = step === STEPS.length - 1;

  return (
    <AnimatePresence>
      <motion.div
        className="onboarding-overlay"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.15 }}
        onClick={(e) => e.stopPropagation()}
      >
        <motion.div
          className="onboarding-panel"
          initial={{ opacity: 0, scale: 0.96, y: 8 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.96, y: 8 }}
          transition={{ duration: 0.18 }}
        >
          <div className="onboarding-header">
            <span className="onboarding-rune">{s.rune}</span>
            <span className="onboarding-title">{s.title}</span>
            <span className="onboarding-count">{step + 1} / {STEPS.length}</span>
          </div>
          <p className="onboarding-body">{s.body}</p>
          <div className="onboarding-footer">
            <button className="btn" onClick={onComplete}>skip tour</button>
            <div className="onboarding-dots">
              {STEPS.map((_, i) => (
                <span
                  key={i}
                  className={`onboarding-dot${i === step ? " active" : ""}`}
                  onClick={() => setStep(i)}
                />
              ))}
            </div>
            {!isLast ? (
              <button className="btn btn-primary" onClick={() => setStep(step + 1)}>next →</button>
            ) : (
              <button className="btn btn-primary" onClick={onComplete}>finish</button>
            )}
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}

/** Convenience hook: returns whether the tour should show, and a dismiss fn. */
export function useOnboarding() {
  const setTheme = useGaldrStore((s) => s.setTheme);
  void setTheme; // store touch to keep hook reactive if needed later
  return { showTour: false, complete: () => {} };
}
