import { useState } from "react";
import ScrambleText from "../components/ScrambleText";

interface Props {
  onNavigate: (page: "convert" | "batch") => void;
}

interface ToolCard {
  rune: string;
  label: string;
  desc: string;
  target: "convert" | "batch";
}

const TOOLS: ToolCard[] = [
  { rune: "ᚨ", label: "convert", desc: "single file conversion", target: "convert" },
  { rune: "ᚷ", label: "batch", desc: "bulk folder conversion", target: "batch" },
];

export default function HomePage({ onNavigate }: Props) {
  const [hoveredCard, setHoveredCard] = useState(-1);

  return (
    <div className="page">
      <div className="home-page-wrapper">
        <div className="home-tagline">ᚱ choose your path</div>
        <div className="home-cards">
          {TOOLS.map((t, i) => (
            <div
              key={t.target}
              className="home-card"
              onClick={() => onNavigate(t.target)}
              onMouseEnter={() => setHoveredCard(i)}
              onMouseLeave={() => setHoveredCard(-1)}
            >
              <ScrambleText as="span" className="home-card-rune" text={t.rune} load ticks={4} trigger={hoveredCard === i} />
              <div className="home-card-body">
                <ScrambleText as="span" className="home-card-label" text={t.label} load ticks={4} trigger={hoveredCard === i} />
                <span className="home-card-desc">{t.desc}</span>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
