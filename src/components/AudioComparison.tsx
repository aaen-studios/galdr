import type { MediaInfo } from "../types";

interface Props {
  originalInfo: MediaInfo;
  compressedInfo: MediaInfo;
}

function fmtSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)}KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}

function fmtKbps(v: number | undefined): string {
  if (!v) return "—";
  return `${(v / 1000).toFixed(0)}kbps`;
}

export default function AudioComparison({ originalInfo, compressedInfo }: Props) {
  const origAudio = originalInfo.streams.find((s) => s.kind === "audio");
  const compAudio = compressedInfo.streams.find((s) => s.kind === "audio");

  if (!origAudio && !compAudio) {
    return (
      <div className="comparison-fallback">
        <span className="comparison-fallback-text">no audio streams found</span>
      </div>
    );
  }

  const rows: { label: string; orig: string; comp: string }[] = [
    {
      label: "codec",
      orig: origAudio?.codec ?? "—",
      comp: compAudio?.codec ?? "—",
    },
    {
      label: "bitrate",
      orig: fmtKbps(origAudio?.bitrate ?? originalInfo.bitrate),
      comp: fmtKbps(compAudio?.bitrate ?? compressedInfo.bitrate),
    },
    {
      label: "sample rate",
      orig: origAudio?.sample_rate ? `${(origAudio.sample_rate / 1000).toFixed(0)}kHz` : "—",
      comp: compAudio?.sample_rate ? `${(compAudio.sample_rate / 1000).toFixed(0)}kHz` : "—",
    },
    {
      label: "channels",
      orig: origAudio?.channels ? `${origAudio.channels}ch` : "—",
      comp: compAudio?.channels ? `${compAudio.channels}ch` : "—",
    },
    {
      label: "file size",
      orig: fmtSize(originalInfo.size),
      comp: fmtSize(compressedInfo.size),
    },
  ];

  const pct = compressedInfo.size < originalInfo.size
    ? `-${Math.round((1 - compressedInfo.size / originalInfo.size) * 100)}%`
    : `+${Math.round((compressedInfo.size / originalInfo.size - 1) * 100)}%`;

  return (
    <div className="audio-comparison">
      <table className="metadata-table">
        <thead>
          <tr>
            <th></th>
            <th>original</th>
            <th>compressed</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.label}>
              <td className="meta-label">{r.label}</td>
              <td className="meta-val">{r.orig}</td>
              <td className="meta-val">{r.comp}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="comparison-pos-info">{pct}</div>
    </div>
  );
}
