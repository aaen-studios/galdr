import { useState, useEffect, useRef, useCallback } from "react";

const RUNES = "ᚠᚢᚦᚨᚱᚲᚷᚹᚺᚾᛁᛃᛇᛈᛉᛊᛏᛒᛖᛗᛚᛝᛟᛞ";
const REVEAL_DURATION = 600;

interface Props {
  text: string;
  as?: "span" | "div" | "h1" | "h2" | "h3" | "p" | "button";
  className?: string;
  hover?: boolean;
  load?: boolean;
  trigger?: boolean;
  ticks?: number;
  onClick?: () => void;
}

function randomRune(): string {
  return RUNES[Math.floor(Math.random() * RUNES.length)];
}

function shuffle(target: string, factor: number): string {
  let out = "";
  for (let i = 0; i < target.length; i++) {
    if (target[i] === " ") {
      out += " ";
    } else if (Math.random() < factor) {
      out += randomRune();
    } else {
      out += target[i];
    }
  }
  return out;
}

export default function ScrambleText({
  text,
  as: Tag = "span",
  className,
  hover = false,
  load = false,
  trigger,
  ticks = 8,
  onClick,
}: Props) {
  const [display, setDisplay] = useState(text);
  const [active, setActive] = useState(false);
  const timerRef = useRef<number>(undefined);
  const prevTrigger = useRef(false);

  const run = useCallback(() => {
    setActive(true);
    let step = 0;
    clearInterval(timerRef.current);
    timerRef.current = setInterval(() => {
      step++;
      const factor = Math.max(0, 1 - step / ticks);
      setDisplay(shuffle(text, factor));
      if (step >= ticks) {
        clearInterval(timerRef.current);
        setDisplay(text);
        setActive(false);
      }
    }, REVEAL_DURATION / ticks);
  }, [text, ticks]);

  useEffect(() => {
    if (load) run();
  }, [load, run]);

  useEffect(() => {
    if (trigger && trigger !== prevTrigger.current) {
      prevTrigger.current = trigger;
      run();
    }
  }, [trigger, run]);

  useEffect(() => {
    if (!hover && trigger === undefined) setDisplay(text);
  }, [text, hover, trigger]);

  return (
    <Tag
      className={className}
      onClick={onClick}
      onMouseEnter={hover && !active ? run : undefined}
      onMouseLeave={hover ? () => { setDisplay(text); setActive(false); clearInterval(timerRef.current); } : undefined}
    >
      {display}
    </Tag>
  );
}
