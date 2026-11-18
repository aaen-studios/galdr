"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { getReleasesPage, type Release } from "@/lib/github";
import { renderMarkdown } from "@/lib/markdown";

const RUNES = ["ᚠ ᚢ ᚦ", "ᚨ ᚱ ᚲ", "ᚷ ᚹ ᚺ", "ᚾ ᛁ ᛃ", "ᛇ ᛈ ᛉ", "ᛊ ᛏ ᛒ", "ᛖ ᛗ ᛚ", "ᛜ ᛞ ᛟ"];
const MONTHS = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII"];

function formatRune(index: number) {
  return RUNES[index % RUNES.length];
}

function formatDate(iso: string) {
  const d = new Date(iso);
  return {
    short: `${String(d.getUTCDate()).padStart(2, "0")} · ${MONTHS[d.getUTCMonth()]} · ${d.getUTCFullYear()}`,
    iso: d.toISOString().slice(0, 10),
  };
}

function ReleaseArticle({ release, index }: { release: Release; index: number }) {
  const date = formatDate(release.published_at);
  return (
    <article className="release reveal in">
      <div className="release-top">
        <span className="rune" aria-hidden="true">
          {formatRune(index)}
        </span>
        <strong>{release.tag_name}</strong>
        <time dateTime={date.iso}>{date.short}</time>
      </div>
      {release.body ? renderMarkdown(release.body) : null}
    </article>
  );
}

/**
 * Folio IV changelog: server-rendered initial releases arrive as props;
 * older folios are conjured on scroll via the GitHub API (5 per page).
 */
export default function Changelog({ initialReleases }: { initialReleases: Release[] }) {
  const [releases, setReleases] = useState<Release[]>(initialReleases);
  const [loading, setLoading] = useState(false);
  const [done, setDone] = useState(initialReleases.length === 0);
  const pageRef = useRef(2);
  const sentinelRef = useRef<HTMLDivElement>(null);

  const loadMore = useCallback(async () => {
    if (loading || done) return;
    setLoading(true);
    const next = await getReleasesPage(pageRef.current);
    if (next.length > 0) {
      setReleases((prev) => [...prev, ...next]);
      pageRef.current += 1;
    }
    if (next.length < 5) setDone(true);
    setLoading(false);
  }, [loading, done]);

  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel) return;
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) loadMore();
      },
      { rootMargin: "0px 0px 480px 0px" }
    );
    io.observe(sentinel);
    return () => io.disconnect();
  }, [loadMore]);

  return (
    <>
      {releases.map((release, i) => (
        <ReleaseArticle key={release.tag_name} release={release} index={i} />
      ))}
      {releases.length === 0 && (
        <p className="earlier">the index is silent — releases could not be fetched.</p>
      )}
      <div ref={sentinelRef} aria-hidden="true" />
      {loading && <p className="earlier">summoning earlier folios…</p>}
      {done && releases.length > 0 && (
        <p className="earlier">✦ here the index ends — older folios are kept in the vault ✦</p>
      )}
    </>
  );
}
